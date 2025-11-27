"""
Tests for the ShoppingPredictor class, specifically testing the fix for
training on missing target_days_until_next values.
"""

import pandas as pd
import numpy as np
import pytest
from datetime import datetime, timedelta

from predictor import ShoppingPredictor, _compute_target_days


class TestComputeTargetDays:
    """Tests for the _compute_target_days helper function."""

    def test_computes_days_between_tickets(self):
        """Should correctly compute days until next visit from fecha_hora."""
        df = pd.DataFrame({
            'fecha_hora': [
                '2024-01-01T10:00:00',
                '2024-01-03T14:00:00',  # 2 days after first
                '2024-01-10T09:00:00',  # 7 days after second
            ],
            'day_of_week': [0, 2, 2],
            'hour_of_day': [10, 14, 9],
            'days_since_last_shop': [0, 2, 7],
            'total_last_30d': [100, 150, 200],
            'tickets_last_30d': [1, 2, 3],
            'is_payday_week': [False, False, True],
            'total': [50, 60, 70],
        })

        result = _compute_target_days(df)

        assert 'target_days_until_next' in result.columns
        # First ticket to second: 2 days + 4 hours = 2 + 4/24 = 2.1667 days
        expected_first = 2 + 4 / 24
        assert abs(result.iloc[0]['target_days_until_next'] - expected_first) < 0.01
        # Second ticket to third: 6 days + 19 hours = 6 + 19/24 = 6.7917 days
        expected_second = 6 + 19 / 24
        assert abs(result.iloc[1]['target_days_until_next'] - expected_second) < 0.01
        # Last ticket has no next visit
        assert pd.isna(result.iloc[2]['target_days_until_next'])

    def test_handles_empty_dataframe(self):
        """Should return empty DataFrame for empty input."""
        df = pd.DataFrame()
        result = _compute_target_days(df)
        assert result.empty

    def test_handles_missing_fecha_hora_column(self):
        """Should return unchanged DataFrame if fecha_hora column is missing."""
        df = pd.DataFrame({'day_of_week': [0, 1, 2]})
        result = _compute_target_days(df)
        assert 'target_days_until_next' not in result.columns

    def test_handles_single_ticket(self):
        """Should handle single ticket (no next visit possible)."""
        df = pd.DataFrame({
            'fecha_hora': ['2024-01-01T10:00:00'],
            'day_of_week': [0],
        })

        result = _compute_target_days(df)

        assert 'target_days_until_next' in result.columns
        assert pd.isna(result.iloc[0]['target_days_until_next'])


class TestShoppingPredictorTrain:
    """Tests for the ShoppingPredictor.train method."""

    def _make_history_df(self, n_tickets=5):
        """Helper to create a realistic history DataFrame without target."""
        base_date = datetime(2024, 1, 1, 10, 0)
        dates = [base_date + timedelta(days=i*3) for i in range(n_tickets)]
        
        return pd.DataFrame({
            'numero_factura': [f'F{i}' for i in range(n_tickets)],
            'fecha_hora': [d.isoformat() for d in dates],
            'total': [50.0 + i*10 for i in range(n_tickets)],
            'day_of_week': [d.weekday() for d in dates],
            'day_of_month': [d.day for d in dates],
            'hour_of_day': [d.hour for d in dates],
            'days_since_last_shop': [0.0] + [3.0] * (n_tickets - 1),
            'total_last_30d': [100.0 + i*20 for i in range(n_tickets)],
            'tickets_last_30d': list(range(1, n_tickets + 1)),
            'is_payday_week': [False] * n_tickets,
            'target_days_until_next': [None] * n_tickets,  # All None, mimics backend
        })

    def test_trains_with_all_nan_targets(self):
        """Should compute targets when all target_days_until_next are NaN/None."""
        predictor = ShoppingPredictor()
        df = self._make_history_df(5)

        # This should NOT raise an error anymore
        predictor.train(df)

        assert predictor.is_trained

    def test_trains_with_missing_target_column(self):
        """Should compute targets when target_days_until_next column is missing."""
        predictor = ShoppingPredictor()
        df = self._make_history_df(5)
        df = df.drop(columns=['target_days_until_next'])

        predictor.train(df)

        assert predictor.is_trained

    def test_trains_and_predicts_successfully(self):
        """Should be able to train and then predict."""
        predictor = ShoppingPredictor()
        df = self._make_history_df(10)

        predictor.train(df)

        assert predictor.is_trained

        # Test prediction
        current_features = {
            'day_of_week': 1,
            'hour_of_day': 12,
            'days_since_last_shop': 3.0,
            'total_last_30d': 200.0,
            'tickets_last_30d': 5,
            'is_payday_week': False,
        }

        result = predictor.predict_next_visit(current_features)

        assert result is not None
        assert 'days_until' in result
        assert 'predicted_hour' in result
        assert 'predicted_spend' in result
        assert isinstance(result['days_until'], float)
        assert isinstance(result['predicted_hour'], int)
        assert isinstance(result['predicted_spend'], float)

    def test_handles_single_ticket_history(self):
        """Should handle training with just one ticket (edge case)."""
        predictor = ShoppingPredictor()
        df = self._make_history_df(1)

        # Single ticket means no valid training data for next_visit model
        # but should not crash
        predictor.train(df)

        # is_trained is still True but models may not be fitted
        assert predictor.is_trained

    def test_handles_empty_history(self):
        """Should handle empty history without crashing."""
        predictor = ShoppingPredictor()
        df = pd.DataFrame()

        predictor.train(df)

        # Should not be trained with empty data
        assert not predictor.is_trained

    def test_preserves_precomputed_valid_targets(self):
        """Should use precomputed targets if they are valid (not all NaN)."""
        predictor = ShoppingPredictor()
        df = self._make_history_df(5)
        # Set some valid targets (not all NaN)
        df['target_days_until_next'] = [3.0, 3.0, 3.0, 3.0, np.nan]

        predictor.train(df)

        assert predictor.is_trained


if __name__ == '__main__':
    pytest.main([__file__, '-v'])
