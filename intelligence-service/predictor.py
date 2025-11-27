import pandas as pd
import numpy as np
from sklearn.ensemble import RandomForestRegressor, RandomForestClassifier
from sklearn.neighbors import KNeighborsClassifier
from datetime import datetime, timedelta
import joblib
import os


def _compute_target_days(df: pd.DataFrame) -> pd.DataFrame:
    """
    Compute target_days_until_next from fecha_hora column.
    
    This function calculates the number of days between each ticket and the next one.
    The last ticket in the sorted list will have NaN as there's no next visit.
    
    Args:
        df: DataFrame with 'fecha_hora' column (ISO format strings or datetime)
        
    Returns:
        DataFrame with 'target_days_until_next' column computed
    """
    if df.empty or 'fecha_hora' not in df.columns:
        return df
    
    # Make a copy to avoid modifying the original
    df = df.copy()
    
    # Convert fecha_hora to datetime if it's a string
    df['_parsed_dt'] = pd.to_datetime(df['fecha_hora'], errors='coerce')
    
    # Sort by datetime to compute days until next visit correctly
    df = df.sort_values('_parsed_dt').reset_index(drop=True)
    
    # Compute days until next visit by shifting the datetime column
    df['_next_dt'] = df['_parsed_dt'].shift(-1)
    df['target_days_until_next'] = (df['_next_dt'] - df['_parsed_dt']).dt.total_seconds() / 86400
    
    # Clean up temporary columns
    df = df.drop(columns=['_parsed_dt', '_next_dt'])
    
    return df


class ShoppingPredictor:
    def __init__(self):
        self.model_next_visit = RandomForestRegressor(n_estimators=100, random_state=42)
        self.model_time_window = KNeighborsClassifier(n_neighbors=5)
        self.model_spend = RandomForestRegressor(n_estimators=100, random_state=42)
        # Dictionary to hold classifiers for each product
        self.product_models = {} 
        self.is_trained = False

    def train(self, ticket_features: pd.DataFrame, product_stats: pd.DataFrame = None):
        """
        Trains the models using historical ticket features.
        ticket_features should contain:
        - days_since_last_shop
        - day_of_week
        - hour_of_day
        - total_last_30d
        - tickets_last_30d
        - is_payday_week
        - target_days_until_next (calculated from fecha_hora if missing)
        - total (target for spend)
        """
        if ticket_features.empty:
            print("No data to train on.")
            return

        # Compute target_days_until_next if missing or all NaN
        needs_target_computation = (
            'target_days_until_next' not in ticket_features.columns
            or ticket_features['target_days_until_next'].isna().all()
        )
        if needs_target_computation:
            ticket_features = _compute_target_days(ticket_features)

        feature_cols = [
            'day_of_week', 'hour_of_day', 'days_since_last_shop', 
            'total_last_30d', 'tickets_last_30d', 'is_payday_week'
        ]

        # Train Next Visit Model (requires valid target)
        if 'target_days_until_next' in ticket_features.columns:
            # Filter out rows with NaN targets (e.g., the last ticket has no next visit)
            valid_mask = ticket_features['target_days_until_next'].notna()
            df_valid = ticket_features[valid_mask]

            if len(df_valid) >= 1:
                X_next = df_valid[feature_cols]
                y_next = df_valid['target_days_until_next']
                self.model_next_visit.fit(X_next, y_next)

                # Train Time Window Model (KNN) on the same valid rows
                self.model_time_window.fit(X_next, df_valid['hour_of_day'])
            else:
                print("Not enough valid data to train next visit model.")

        # Train Spend Model (uses all rows with valid total)
        if 'total' in ticket_features.columns:
            spend_mask = ticket_features['total'].notna()
            df_spend = ticket_features[spend_mask]
            if len(df_spend) >= 1:
                X_spend = df_spend[feature_cols]
                y_spend = df_spend['total']
                self.model_spend.fit(X_spend, y_spend)
            else:
                print("Not enough valid data to train spend model.")

        self.is_trained = True
        print("Models trained successfully.")

    def predict_next_visit(self, current_features: dict):
        if not self.is_trained:
            return None

        # Convert dict to DataFrame
        X_new = pd.DataFrame([current_features])
        
        # Predict days until next shop
        days_until = self.model_next_visit.predict(X_new)[0]
        
        # Predict likely hour
        predicted_hour = self.model_time_window.predict(X_new)[0]
        
        # Predict spend
        predicted_spend = self.model_spend.predict(X_new)[0]

        return {
            "days_until": float(days_until),
            "predicted_hour": int(predicted_hour),
            "predicted_spend": float(predicted_spend)
        }

    def save(self, path="model_dump.joblib"):
        joblib.dump(self, path)

    @staticmethod
    def load(path="model_dump.joblib"):
        if os.path.exists(path):
            return joblib.load(path)
        return ShoppingPredictor()
