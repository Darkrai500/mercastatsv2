from fastapi import FastAPI, HTTPException
from pydantic import BaseModel
from typing import List, Optional
import pandas as pd
from predictor import ShoppingPredictor
import os

app = FastAPI(title="Mercastats Intelligence Service")

# Global predictor instance
predictor = ShoppingPredictor.load()

class TicketFeature(BaseModel):
    numero_factura: Optional[str]
    fecha_hora: Optional[str] # ISO format
    total: Optional[float]
    day_of_week: int
    day_of_month: int
    hour_of_day: int
    days_since_last_shop: float
    total_last_30d: float
    tickets_last_30d: int
    is_payday_week: bool
    # Targets for training (optional in inference)
    target_days_until_next: Optional[float] = None

class PredictRequest(BaseModel):
    user_id: str
    current_date: str
    features_now: TicketFeature
    history_features: List[TicketFeature]

class PredictionResponse(BaseModel):
    timestamp: str
    time_window_label: str
    estimated_total: float
    confidence: float
    suggested_products: List[dict]

@app.post("/predict/next")
async def predict_next(request: PredictRequest):
    # 1. Train/Update model with history (in a real app, this might be async or scheduled)
    # For this MVP, we'll retrain on the fly if history is provided and model is not trained
    # or just use the history to build the context.
    
    # Convert history to DataFrame
    if request.history_features:
        df_history = pd.DataFrame([vars(f) for f in request.history_features])
        # Simple training trigger for demo purposes
        # In production, we would load a pre-trained model per user or global
        predictor.train(df_history)
    
    if not predictor.is_trained:
        raise HTTPException(status_code=503, detail="Model not trained yet")

    # 2. Predict
    current_features = vars(request.features_now)
    # Remove fields not used by model if necessary, or let the model handle it
    # The model expects specific keys, we need to ensure they match
    model_input = {
        'day_of_week': current_features['day_of_week'],
        'hour_of_day': current_features['hour_of_day'],
        'days_since_last_shop': current_features['days_since_last_shop'],
        'total_last_30d': current_features['total_last_30d'],
        'tickets_last_30d': current_features['tickets_last_30d'],
        'is_payday_week': current_features['is_payday_week']
    }
    
    result = predictor.predict_next_visit(model_input)
    
    # 3. Format response
    # Calculate predicted date
    from datetime import datetime, timedelta
    current_dt = datetime.fromisoformat(request.current_date)
    predicted_date = current_dt + timedelta(days=result['days_until'])
    # Set the hour
    predicted_date = predicted_date.replace(hour=result['predicted_hour'], minute=0, second=0)
    
    return {
        "prediction": {
            "timestamp": predicted_date.isoformat(),
            "time_window_label": f"Estimated around {result['predicted_hour']}:00",
            "estimated_total": round(result['predicted_spend'], 2),
            "confidence": 0.85, # Placeholder
            "suggested_products": [] # Placeholder for now
        }
    }

@app.get("/health")
def health_check():
    return {"status": "ok", "model_loaded": predictor.is_trained}
