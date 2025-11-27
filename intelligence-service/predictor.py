import pandas as pd
import numpy as np
from sklearn.ensemble import RandomForestRegressor, RandomForestClassifier
from sklearn.neighbors import KNeighborsClassifier
from datetime import datetime, timedelta
import joblib
import os

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
        - target_days_until_next (calculated)
        - total (target for spend)
        """
        if ticket_features.empty:
            print("No data to train on.")
            return

        # Prepare features for Next Visit Model
        X = ticket_features[[
            'day_of_week', 'hour_of_day', 'days_since_last_shop', 
            'total_last_30d', 'tickets_last_30d', 'is_payday_week'
        ]]
        
        # Target for Next Visit: days until next shop
        # We need to calculate this from the data if not present, but the plan says 
        # we might calculate it here. Let's assume the input DF has it or we calculate it.
        # For simplicity, let's assume the input dataframe is prepared with targets.
        
        if 'target_days_until_next' in ticket_features.columns:
            y_next = ticket_features['target_days_until_next']
            self.model_next_visit.fit(X, y_next)
            
            # Train Time Window Model (KNN)
            # Target: hour_of_day of the NEXT visit
            # We need to shift the target to be the next visit's hour
            # But for simplicity in this initial version, let's assume we train on current patterns
            # actually, the plan says: "Input: contexto temporal ... + salida del modelo de días"
            # So we might need a more complex training setup.
            # For now, let's train a simple classifier for hour based on current context
            self.model_time_window.fit(X, ticket_features['hour_of_day']) 

        if 'total' in ticket_features.columns:
            y_spend = ticket_features['total']
            self.model_spend.fit(X, y_spend)

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
