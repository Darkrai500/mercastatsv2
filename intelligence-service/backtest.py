"""
Backtest del predictor con usuarios SINTÉTICOS (no hay datos reales en el repositorio).

Para cada usuario simulado se recorre su historial en orden: en cada paso el modelo solo
ve el pasado, el "ahora" cae en un instante aleatorio entre la última visita y la
siguiente (real, desconocida para el modelo) y se compara la predicción con lo que pasó.

Sirve para detectar regresiones y para ajustar los parámetros de ``predictor.py``; los
números absolutos dependen de lo realistas que sean los arquetipos, no son una garantía
sobre usuarios reales.

    python backtest.py              # informe completo
    python backtest.py --seeds 5    # menos usuarios por arquetipo (más rápido)
"""

from __future__ import annotations

import argparse
import random
import statistics
from dataclasses import dataclass
from datetime import datetime, timedelta

from predictor import (
    MIN_VISITS_FOR_PREDICTION,
    ProductLine,
    Ticket,
    predict_next_purchase,
)

SUNDAY = 6


# ============================================================================
# SIMULACIÓN
# ============================================================================


@dataclass
class SimVisit:
    when: datetime
    total: float
    basket: list[ProductLine]


def _clip_hour(hour: float) -> float:
    return min(21.0, max(9.25, hour))


def _at(day: datetime, hour: float) -> datetime:
    hour = _clip_hour(hour)
    return day.replace(hour=int(hour), minute=int((hour % 1) * 60), second=0, microsecond=0)


def _skip_sunday(day: datetime) -> datetime:
    return day + timedelta(days=1) if day.weekday() == SUNDAY else day


def _pattern_dates(rng, start, n, weekdays, hours, p_skip=0.1, p_shift=0.12, hour_sd=0.6, p_optional=None):
    """Visitas que siguen un patrón semanal con saltos y desplazamientos ocasionales."""
    dates = []
    day = start
    optional = p_optional or {}
    while len(dates) < n:
        day += timedelta(days=1)
        if day.weekday() not in weekdays:
            continue
        if day.weekday() in optional and rng.random() > optional[day.weekday()]:
            continue  # día opcional: solo se compra con esa probabilidad
        if rng.random() < p_skip:
            continue
        visit_day = day
        if rng.random() < p_shift:
            visit_day += timedelta(days=rng.choice([-1, 1]))
        visit_day = _skip_sunday(visit_day)
        hour = rng.gauss(hours[weekdays.index(day.weekday())], hour_sd)
        when = _at(visit_day, hour)
        if dates and when <= dates[-1]:
            continue
        dates.append(when)
    return dates


def simulate_dates(archetype: str, n: int, rng: random.Random) -> list[datetime]:
    start = datetime(2025, 9, 1)  # lunes
    if archetype == "lun_jue":
        return _pattern_dates(rng, start, n, [0, 3], [18.5, 18.5])
    if archetype == "lun_jue_horas_distintas":
        return _pattern_dates(rng, start, n, [0, 3], [18.0, 11.0])
    if archetype == "sabado_y_extra":
        return _pattern_dates(rng, start, n, [2, 5], [19.0, 11.0], p_optional={2: 0.3})
    if archetype == "lun_mie_vie":
        return _pattern_dates(rng, start, n, [0, 2, 4], [17.5, 18.0, 19.0], p_skip=0.12)
    if archetype == "quincenal_viernes":
        dates, day = [], start + timedelta(days=4)
        while len(dates) < n:
            visit = day + timedelta(days=rng.choice([0, 0, 0, 1, -1]))
            dates.append(_at(_skip_sunday(visit), rng.gauss(17.0, 0.8)))
            day += timedelta(days=14)
        return dates
    if archetype == "irregular":
        dates, day = [], start
        while len(dates) < n:
            day += timedelta(days=rng.choices([1, 2, 3, 4, 5, 6, 7, 8, 9], [2, 4, 5, 5, 4, 3, 3, 2, 1])[0])
            dates.append(_at(_skip_sunday(day), rng.uniform(10, 20)))
        return dates
    raise ValueError(archetype)


PRODUCTS = [
    # nombre, periodo medio (días), precio
    ("PAN BARRA", 3.5, 0.80),
    ("LECHE ENTERA", 7, 0.95),
    ("HUEVOS M-L", 10, 2.35),
    ("YOGUR NATURAL", 7, 2.10),
    ("ACEITE OLIVA 1L", 35, 7.90),
    ("ARROZ LARGO", 28, 1.45),
    ("PAPEL HIGIENICO", 21, 4.60),
    ("PLATANO", 6, 1.80),
]
RANDOM_PRODUCTS = [("CHOCOLATE NEGRO", 0.12, 1.50), ("PIZZA FRESCA", 0.10, 2.99), ("CERVEZA", 0.08, 5.20)]


def simulate(archetype: str, n: int, seed: int) -> list[SimVisit]:
    rng = random.Random(seed)
    dates = simulate_dates(archetype, n, rng)
    next_need = {name: dates[0] + timedelta(days=rng.uniform(0, period)) for name, period, _ in PRODUCTS}
    visits: list[SimVisit] = []
    for i, when in enumerate(dates):
        gap = (when - dates[i - 1]).days if i else 4
        basket: list[ProductLine] = []
        for name, period, price in PRODUCTS:
            if when >= next_need[name] - timedelta(days=rng.uniform(0, 2)):
                basket.append(ProductLine(name, when, 1.0, price))
                next_need[name] = when + timedelta(days=max(1.0, rng.gauss(period, 0.2 * period)))
        for name, prob, price in RANDOM_PRODUCTS:
            if rng.random() < prob:
                basket.append(ProductLine(name, when, 1.0, price))
        base = sum(l.unit_price * l.quantity for l in basket)
        filler = max(5.0, rng.gauss(30, 8)) * (0.6 + 0.4 * min(gap, 8) / 4.0)
        visits.append(SimVisit(when, round(base + filler, 2), basket))
    return visits


# ============================================================================
# EVALUACIÓN
# ============================================================================


def evaluate(archetype: str, seeds: int, n: int = 70, start_at: int = 12, seed_offset: int = 0) -> dict:
    day_err, hour_err, spend_err, covered, conf, near_hit, exact_hit = [], [], [], [], [], [], []
    base_day_err = []
    prec, rec, old_prec = [], [], []
    for seed in range(seed_offset, seed_offset + seeds):
        visits = simulate(archetype, n, seed)
        rng = random.Random(10_000 + seed)
        for i in range(start_at, len(visits) - 1):
            history = visits[:i]
            truth = visits[i]
            last = history[-1].when
            now = last + (truth.when - last) * rng.uniform(0.1, 0.95)

            tickets = [Ticket(v.when, v.total) for v in history]
            lines = [l for v in history[-60:] for l in v.basket]
            pred = predict_next_purchase(tickets, lines, now)
            if pred.learning_mode:
                continue

            d_err = (pred.when.date() - truth.when.date()).days
            day_err.append(abs(d_err))
            exact_hit.append(d_err == 0)
            near_hit.append(abs(d_err) <= 1)
            hour_err.append(abs((pred.when.hour + pred.when.minute / 60) - (truth.when.hour + truth.when.minute / 60)))
            spend_err.append(abs(pred.total - truth.total))
            covered.append(pred.total_min <= truth.total <= pred.total_max)
            conf.append(pred.confidence)

            # Referencia: último ticket + mediana de huecos.
            gaps = [(b.when.date() - a.when.date()).days for a, b in zip(history, history[1:])][-20:]
            base_day = last.date() + timedelta(days=round(statistics.median(gaps)))
            base_day_err.append(abs((base_day - truth.when.date()).days))

            actual = {l.name for l in truth.basket}
            suggested = [p.name for p in pred.products]
            if suggested:
                hits = len(actual & set(suggested))
                prec.append(hits / len(suggested))
                rec.append(hits / max(1, len(actual)))
                # Referencia anterior: los K productos más comprados en total.
                counts: dict[str, float] = {}
                for l in lines:
                    counts[l.name] = counts.get(l.name, 0.0) + l.quantity
                top = [n_ for n_, _ in sorted(counts.items(), key=lambda kv: -kv[1])[: len(suggested)]]
                old_prec.append(len(actual & set(top)) / len(top))

    def mean(xs):
        return statistics.fmean(xs) if xs else float("nan")

    return {
        "n": len(day_err),
        "dia_exacto": mean(exact_hit),
        "dia_±1": mean(near_hit),
        "mae_dias": mean(day_err),
        "mae_dias_base": mean(base_day_err),
        "mae_hora": mean(hour_err),
        "mae_gasto": mean(spend_err),
        "cobertura_gasto": mean(covered),
        "confianza_media": mean(conf),
        "prec_productos": mean(prec),
        "recall_productos": mean(rec),
        "prec_top_cantidad": mean(old_prec),
    }


ARCHETYPES = [
    "lun_jue",
    "lun_jue_horas_distintas",
    "sabado_y_extra",
    "lun_mie_vie",
    "quincenal_viernes",
    "irregular",
]


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--seeds", type=int, default=12)
    parser.add_argument("--seed-offset", type=int, default=0)
    args = parser.parse_args()

    print(f"(sintético; {args.seeds} usuarios x arquetipo; mínimo de visitas = {MIN_VISITS_FOR_PREDICTION})")
    header = (
        f"{'arquetipo':26}{'n':>6}{'día=':>7}{'día±1':>7}{'MAEd':>6}{'MAEd*':>7}"
        f"{'MAEh':>6}{'MAE€':>7}{'cob€':>6}{'conf':>6}{'precP':>7}{'recP':>6}{'precTop':>8}"
    )
    print(header)
    print("-" * len(header))
    for arch in ARCHETYPES:
        r = evaluate(arch, args.seeds, seed_offset=args.seed_offset)
        print(
            f"{arch:26}{r['n']:>6}{r['dia_exacto']:>7.2f}{r['dia_±1']:>7.2f}{r['mae_dias']:>6.2f}"
            f"{r['mae_dias_base']:>7.2f}{r['mae_hora']:>6.2f}{r['mae_gasto']:>7.2f}{r['cobertura_gasto']:>6.2f}"
            f"{r['confianza_media']:>6.2f}{r['prec_productos']:>7.2f}{r['recall_productos']:>6.2f}"
            f"{r['prec_top_cantidad']:>8.2f}"
        )
    print("\nMAEd* = error en días de la referencia 'último ticket + mediana de huecos'.")
    print("precP/recP = precisión/recall de los productos sugeridos; precTop = precisión de 'los más comprados'.")
    print("conf debería parecerse a día±1 (es la probabilidad de acertar el día ±1, atenuada con pocos datos).")


if __name__ == "__main__":
    main()
