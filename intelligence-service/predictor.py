"""
Predictor de la próxima compra: cuándo, cuánto y qué.

Por qué un modelo estadístico y no un bosque aleatorio / KNN
------------------------------------------------------------
Cada usuario aporta, como mucho, unas decenas de compras. Con tan pocas filas un
RandomForest o un KNN no generalizan, cambian mucho ante pequeñas variaciones del
historial y fallan directamente cuando hay menos filas que vecinos. Los hábitos de
compra, en cambio, tienen una estructura muy marcada (días de la semana preferidos,
intervalos entre visitas, cadencia de cada producto) que se estima de forma directa,
explicable y en microsegundos.

El modelo no guarda estado: se ajusta en cada petición con el historial del propio
usuario, de modo que los datos de un usuario no pueden influir en la predicción de otro.

Convenciones
------------
* Todas las fechas son "hora local de pared" sin zona (la que figura en el ticket).
* Los días de la semana siguen ``date.weekday()``: lunes=0 ... domingo=6.
* Una *visita* agrupa los tickets del mismo día natural (dos tickets el mismo día son
  casi siempre la misma salida a comprar).
"""

from __future__ import annotations

import math
import statistics
from collections import defaultdict
from dataclasses import dataclass
from datetime import date, datetime, timedelta
from typing import Iterable, Sequence

# ============================================================================
# PARÁMETROS
# ============================================================================

# Visitas mínimas (días con compra) para publicar una predicción. Con menos de cinco
# solo hay tres o cuatro intervalos y cualquier "patrón" sería ruido.
MIN_VISITS_FOR_PREDICTION = 5

# Cuántos días hacia delante se consideran candidatos para la próxima visita.
HORIZON_DAYS = 21

# Peso de una visita por cada visita más reciente que ella (olvido gradual).
RECENCY_DECAY = 0.95
# Peso con el que cuentan los intervalos que partían de un día de la semana distinto
# al de la última visita (los del mismo día cuentan 1).
OTHER_WEEKDAY_WEIGHT = 0.3
# Anchura (en días) del núcleo gaussiano con el que se suavizan los intervalos.
GAP_KERNEL_BANDWIDTH = 0.6
# Suavizado de Laplace de la frecuencia por día de la semana.
WEEKDAY_PRIOR_STRENGTH = 0.5
# Cuánto pesa la frecuencia de cada día de la semana frente al patrón de intervalos.
WEEKDAY_PRIOR_EXPONENT = 1.0
# Masa de la distribución a priori geométrica que evita probabilidades exactamente nulas.
GAP_PRIOR_MASS = 0.05
# Si menos de este porcentaje de visitas cae en domingo, se descarta el domingo
# (Mercadona cierra los domingos).
MIN_SUNDAY_SHARE = 0.10

# Horario comercial: la hora estimada se limita a este rango.
STORE_OPEN_HOUR = 9.0
STORE_LAST_ENTRY_HOUR = 21.0
STORE_CLOSE_HOUR = 21.5

# Gasto.
SPEND_RECENCY_DECAY = 0.93
SPEND_MAX_VISITS = 40
SPEND_REGRESSION_MIN_VISITS = 10
SPEND_REGRESSION_MIN_CORRELATION = 0.3
# Semianchura del intervalo de gasto, en desviaciones robustas (≈70 % de cobertura).
SPEND_INTERVAL_SIGMAS = 1.3

# Productos.
PRODUCT_MIN_VISITS = 3
PRODUCT_MIN_PROBABILITY = 0.30
PRODUCT_MAX_SUGGESTIONS = 8
PRODUCT_MAX_PROBABILITY = 0.95
PRODUCT_RECENT_VISITS = 12
PRODUCT_MAX_INTERVALS = 8

SUNDAY = 6


# ============================================================================
# TIPOS
# ============================================================================


@dataclass(frozen=True)
class Ticket:
    when: datetime
    total: float


@dataclass(frozen=True)
class ProductLine:
    name: str
    when: datetime
    quantity: float
    unit_price: float


@dataclass(frozen=True)
class Visit:
    day: date
    hour: float  # hora decimal del ticket principal del día (18.5 = 18:30)
    total: float  # suma de los tickets del día


@dataclass(frozen=True)
class ProductSuggestion:
    name: str
    probability: float
    price_estimation: float
    reason: str


@dataclass(frozen=True)
class Prediction:
    when: datetime  # fecha y hora local estimadas de la próxima visita
    window_start: float  # horas decimales
    window_end: float
    day_probability: float  # P(la visita es exactamente ese día)
    confidence: float  # P(acertar el día ±1) atenuada si hay pocos datos
    total: float
    total_min: float
    total_max: float
    visits_used: int
    learning_mode: bool
    products: tuple[ProductSuggestion, ...] = ()


# ============================================================================
# UTILIDADES
# ============================================================================


def group_visits(tickets: Iterable[Ticket]) -> list[Visit]:
    """Agrupa tickets por día natural y devuelve las visitas en orden cronológico."""
    by_day: dict[date, list[Ticket]] = defaultdict(list)
    for ticket in tickets:
        by_day[ticket.when.date()].append(ticket)

    visits = []
    for day in sorted(by_day):
        day_tickets = by_day[day]
        main = max(day_tickets, key=lambda t: t.total)
        visits.append(
            Visit(
                day=day,
                hour=main.when.hour + main.when.minute / 60.0,
                total=sum(t.total for t in day_tickets),
            )
        )
    return visits


def _recency_weights(n: int, decay: float) -> list[float]:
    """Pesos 1 para el último elemento y ``decay**k`` para el k-ésimo más antiguo."""
    return [decay ** (n - 1 - i) for i in range(n)]


def _weighted_quantile(values: Sequence[float], weights: Sequence[float], q: float) -> float:
    pairs = sorted(zip(values, weights))
    total = sum(w for _, w in pairs)
    if total <= 0:
        return statistics.median(values)
    target = q * total
    acc = 0.0
    for value, weight in pairs:
        acc += weight
        if acc >= target:
            return value
    return pairs[-1][0]


def _norm_cdf(z: float) -> float:
    return 0.5 * (1.0 + math.erf(z / math.sqrt(2.0)))


def _round_to_quarter(hour: float) -> float:
    return round(hour * 4.0) / 4.0


# ============================================================================
# CUÁNDO
# ============================================================================


@dataclass(frozen=True)
class NextVisit:
    day: date
    hour: float
    window_start: float
    window_end: float
    day_probability: float
    near_probability: float  # P(día ±1)


class NextVisitModel:
    """
    Distribución de probabilidad sobre el día de la próxima visita.

    Cada día candidato ``D`` (a ``g`` días de la última visita) puntúa como:

        score(D) = [ Σ_i peso_i · K(g − g_i) · c(día_origen_i)  +  prior(g) ] · P(día_semana(D))

    donde ``g_i`` son los intervalos históricos entre visitas, ``K`` un núcleo gaussiano,
    ``c`` favorece los intervalos que partían del mismo día de la semana que la última
    visita ("tras un jueves suele venir un lunes") y ``P(día_semana)`` es la frecuencia
    suavizada de cada día de la semana.
    """

    def __init__(self, visits: Sequence[Visit]):
        if not visits:
            raise ValueError("Se necesita al menos una visita")
        self.visits = list(visits)
        count = len(self.visits)
        self._weights = _recency_weights(count, RECENCY_DECAY)

        self._pairs: list[tuple[int, int, float]] = []
        for i in range(count - 1):
            gap = (self.visits[i + 1].day - self.visits[i].day).days
            self._pairs.append((self.visits[i].day.weekday(), gap, self._weights[i + 1]))

        counts = [WEEKDAY_PRIOR_STRENGTH] * 7
        sundays = 0
        for visit, weight in zip(self.visits, self._weights):
            counts[visit.day.weekday()] += weight
            sundays += visit.day.weekday() == SUNDAY
        total = sum(counts)
        self._weekday_prior = [c / total for c in counts]
        self._allow_sunday = sundays / count >= MIN_SUNDAY_SHARE

        if self._pairs:
            pair_weight = sum(w for _, _, w in self._pairs)
            self._mean_gap = max(
                1.0, sum(g * w for _, g, w in self._pairs) / pair_weight
            )
            self._pair_weight = pair_weight
        else:
            self._mean_gap = 7.0
            self._pair_weight = 1.0

        self._hour_cache: dict[int, tuple[float, float, float]] = {}

    # -- día -----------------------------------------------------------------

    def _gap_score(self, gap: int, last_weekday: int) -> float:
        score = 0.0
        for weekday, observed_gap, weight in self._pairs:
            z = (gap - observed_gap) / GAP_KERNEL_BANDWIDTH
            if abs(z) > 4.0:
                continue
            affinity = 1.0 if weekday == last_weekday else OTHER_WEEKDAY_WEIGHT
            score += weight * affinity * math.exp(-0.5 * z * z)
        prior = math.exp(-gap / self._mean_gap) / self._mean_gap
        return score + GAP_PRIOR_MASS * self._pair_weight * prior

    # -- hora ----------------------------------------------------------------

    def typical_hour(self, weekday: int) -> tuple[float, float, float]:
        """(hora más habitual, inicio, fin) de la ventana para ese día de la semana."""
        cached = self._hour_cache.get(weekday)
        if cached is not None:
            return cached

        pool = [
            (v.hour, w)
            for v, w in zip(self.visits, self._weights)
            if v.day.weekday() == weekday
        ]
        if len(pool) < 2:
            # Poca historia para ese día: se completa con el resto de días, con menos peso.
            pool = [
                (v.hour, w * (1.0 if v.day.weekday() == weekday else 0.5))
                for v, w in zip(self.visits, self._weights)
            ]

        step = 0.25
        grid = [
            STORE_OPEN_HOUR + step * k
            for k in range(int((STORE_LAST_ENTRY_HOUR - STORE_OPEN_HOUR) / step) + 1)
        ]
        best_x, best_density = grid[0], -1.0
        for x in grid:
            density = sum(w * math.exp(-0.5 * (x - h) ** 2) for h, w in pool)
            if density > best_density:
                best_x, best_density = x, density

        peak = _round_to_quarter(best_x)
        start = max(STORE_OPEN_HOUR, peak - 1.0)
        end = min(STORE_CLOSE_HOUR, peak + 1.0)
        result = (peak, start, end)
        self._hour_cache[weekday] = result
        return result

    # -- predicción ----------------------------------------------------------

    def predict(self, now: datetime) -> NextVisit:
        last = self.visits[-1]
        today = now.date()
        now_hour = now.hour + now.minute / 60.0
        first_candidate = max(today, last.day + timedelta(days=1))

        candidates: list[tuple[date, float]] = []
        for offset in range(HORIZON_DAYS + 1):
            day = first_candidate + timedelta(days=offset)
            weekday = day.weekday()
            if weekday == SUNDAY and not self._allow_sunday:
                continue
            if day == today:
                # Hoy solo cuenta si la hora habitual todavía no ha pasado.
                if self.typical_hour(weekday)[0] <= now_hour + 0.25:
                    continue
            gap = (day - last.day).days
            score = self._gap_score(gap, last.day.weekday()) * (
                self._weekday_prior[weekday] ** WEEKDAY_PRIOR_EXPONENT
            )
            candidates.append((day, score))

        if not candidates:
            day = first_candidate
            peak, start, end = self.typical_hour(day.weekday())
            return NextVisit(day, peak, start, end, 0.0, 0.0)

        total = sum(score for _, score in candidates)
        if total <= 0:
            probabilities = {day: 1.0 / len(candidates) for day, _ in candidates}
        else:
            probabilities = {day: score / total for day, score in candidates}

        best_day = max(probabilities, key=lambda d: (probabilities[d], -d.toordinal()))
        near = sum(
            probabilities.get(best_day + timedelta(days=delta), 0.0) for delta in (-1, 0, 1)
        )
        peak, start, end = self.typical_hour(best_day.weekday())
        return NextVisit(best_day, peak, start, end, probabilities[best_day], near)


# ============================================================================
# CUÁNTO
# ============================================================================


def estimate_total(visits: Sequence[Visit], gap_days: float) -> tuple[float, float, float]:
    """
    Gasto esperado (central, mínimo, máximo) de la próxima visita.

    Parte de la mediana ponderada por recencia de las últimas visitas. Con suficiente
    historial y solo si existe relación real, ajusta por el tiempo transcurrido desde
    la visita anterior (cuanto más tiempo, más grande suele ser la compra).
    """
    recent = list(visits)[-SPEND_MAX_VISITS:]
    count = len(recent)
    totals = [v.total for v in recent]
    weights = _recency_weights(count, SPEND_RECENCY_DECAY)

    center = _weighted_quantile(totals, weights, 0.5)
    residuals = [t - center for t in totals]
    residual_weights = weights

    if count >= SPEND_REGRESSION_MIN_VISITS:
        xs, ys, ws = [], [], []
        for i in range(1, count):
            gap = max(1, (recent[i].day - recent[i - 1].day).days)
            xs.append(math.log(gap))
            ys.append(totals[i])
            ws.append(weights[i])
        w_sum = sum(ws)
        mean_x = sum(w * x for w, x in zip(ws, xs)) / w_sum
        mean_y = sum(w * y for w, y in zip(ws, ys)) / w_sum
        sxx = sum(w * (x - mean_x) ** 2 for w, x in zip(ws, xs))
        syy = sum(w * (y - mean_y) ** 2 for w, y in zip(ws, ys))
        sxy = sum(w * (x - mean_x) * (y - mean_y) for w, x, y in zip(ws, xs, ys))
        if sxx > 1e-9 and syy > 1e-9:
            correlation = sxy / math.sqrt(sxx * syy)
            if abs(correlation) >= SPEND_REGRESSION_MIN_CORRELATION:
                shrink = len(xs) / (len(xs) + 10.0)  # contracción hacia "sin efecto"
                slope = shrink * sxy / sxx
                center = mean_y + slope * (math.log(max(1.0, gap_days)) - mean_x)
                residuals = [y - (mean_y + slope * (x - mean_x)) for x, y in zip(xs, ys)]
                residual_weights = ws

    center = min(max(center, min(totals)), max(totals))
    center = max(center, 0.0)

    mad = _weighted_quantile([abs(r) for r in residuals], residual_weights, 0.5)
    sigma = max(1.4826 * mad, 0.05 * center)
    if count < 5:
        sigma = max(sigma, 0.25 * center)

    low = max(0.0, center - SPEND_INTERVAL_SIGMAS * sigma)
    high = center + SPEND_INTERVAL_SIGMAS * sigma
    return round(center, 2), round(low, 2), round(high, 2)


# ============================================================================
# QUÉ
# ============================================================================


def _typical_visit_gap(days: Sequence[date]) -> float:
    if len(days) < 3:
        return 3.5
    gaps = [(b - a).days for a, b in zip(days, days[1:])]
    return max(1.0, statistics.median(gaps))


def suggest_products(
    lines: Iterable[ProductLine],
    predicted_day: date,
    max_items: int = PRODUCT_MAX_SUGGESTIONS,
    min_probability: float = PRODUCT_MIN_PROBABILITY,
) -> list[ProductSuggestion]:
    """
    Productos con más probabilidad de estar en la próxima compra.

    Para cada producto se estima su cadencia de recompra (mediana de los días entre
    compras). La probabilidad combina:

    * cuánto "toca" comprarlo en la fecha prevista (según el tiempo transcurrido desde
      la última vez frente a su cadencia habitual), y
    * la frecuencia con que aparece en las últimas visitas.

    Cuanto más irregular es un producto, más pesa la frecuencia y menos la cadencia.
    """
    per_product: dict[str, dict[date, list[ProductLine]]] = defaultdict(lambda: defaultdict(list))
    visit_days: set[date] = set()
    for line in lines:
        per_product[line.name][line.when.date()].append(line)
        visit_days.add(line.when.date())

    ordered_days = sorted(visit_days)
    if len(ordered_days) < PRODUCT_MIN_VISITS:
        return []

    half_gap = _typical_visit_gap(ordered_days) / 2.0
    recent_days = set(ordered_days[-PRODUCT_RECENT_VISITS:])

    suggestions: list[tuple[float, ProductSuggestion]] = []
    for name, by_day in per_product.items():
        days = sorted(by_day)
        purchases = len(days)
        if purchases < 2:
            continue

        intervals = [(b - a).days for a, b in zip(days, days[1:])][-PRODUCT_MAX_INTERVALS:]
        mu = max(1.0, statistics.median(intervals))
        mad = statistics.median([abs(i - mu) for i in intervals])
        sigma = max(1.4826 * mad, 0.2 * mu, 0.75)

        elapsed = max(0, (predicted_day - days[-1]).days)
        due = _norm_cdf((elapsed + half_gap - mu) / sigma)
        stale_after = max(3.0 * mu, 45.0)
        if elapsed > stale_after:
            due *= math.exp(-(elapsed - stale_after) / max(mu, 14.0))

        recent_hits = sum(1 for d in days if d in recent_days)
        window = min(len(ordered_days), PRODUCT_RECENT_VISITS)
        frequency = (recent_hits + 0.5) / (window + 1.0)

        regularity = max(0.0, 1.0 - sigma / mu)
        reliability = min(1.0, (purchases - 1) / (purchases + 2.0)) * regularity
        probability = min(
            PRODUCT_MAX_PROBABILITY, reliability * due + (1.0 - reliability) * frequency
        )
        if probability < min_probability:
            continue

        quantities = [sum(l.quantity for l in by_day[d]) for d in days]
        typical_quantity = statistics.median(quantities)
        if abs(typical_quantity - round(typical_quantity)) < 0.05:
            typical_quantity = max(1, round(typical_quantity))
        last_price = by_day[days[-1]][-1].unit_price
        price = round(last_price * typical_quantity, 2)

        if reliability >= 0.35:
            reason = (
                f"Lo compras cada ~{round(mu)} días; para entonces habrán pasado {elapsed} "
                "desde la última vez"
            )
        else:
            reason = f"Aparece en el {round(frequency * 100)}% de tus últimas compras"

        suggestions.append(
            (probability, ProductSuggestion(name, round(probability, 2), price, reason))
        )

    suggestions.sort(key=lambda item: (-item[0], item[1].name))
    return [s for _, s in suggestions[:max_items]]


# ============================================================================
# PUNTO DE ENTRADA
# ============================================================================


def _learning_prediction(visits: Sequence[Visit], now: datetime) -> Prediction:
    """Respuesta neutra cuando aún no hay historial suficiente para detectar un patrón."""
    when = datetime.combine(now.date() + timedelta(days=7), datetime.min.time()).replace(hour=18)
    total = round(statistics.median([v.total for v in visits]), 2) if visits else 0.0
    return Prediction(
        when=when,
        window_start=17.0,
        window_end=19.0,
        day_probability=0.0,
        confidence=0.0,
        total=total,
        total_min=total,
        total_max=total,
        visits_used=len(visits),
        learning_mode=True,
    )


def predict_next_purchase(
    tickets: Sequence[Ticket],
    product_lines: Sequence[ProductLine],
    now: datetime,
) -> Prediction:
    """
    Predice cuándo irá el usuario a comprar, cuánto gastará y qué llevará.

    ``now`` y los tickets están en hora local de pared (sin zona).
    """
    visits = group_visits(tickets)
    if len(visits) < MIN_VISITS_FOR_PREDICTION:
        return _learning_prediction(visits, now)

    model = NextVisitModel(visits)
    nxt = model.predict(now)

    gap = max(1, (nxt.day - visits[-1].day).days)
    total, total_min, total_max = estimate_total(visits, gap)

    data_factor = 1.0 - math.exp(-len(visits) / 10.0)
    confidence = round(data_factor * nxt.near_probability, 2)

    hours = int(nxt.hour)
    minutes = int(round((nxt.hour - hours) * 60))
    when = datetime.combine(nxt.day, datetime.min.time()).replace(hour=hours, minute=minutes)

    return Prediction(
        when=when,
        window_start=nxt.window_start,
        window_end=nxt.window_end,
        day_probability=round(nxt.day_probability, 3),
        confidence=confidence,
        total=total,
        total_min=total_min,
        total_max=total_max,
        visits_used=len(visits),
        learning_mode=False,
        products=tuple(suggest_products(product_lines, nxt.day)),
    )
