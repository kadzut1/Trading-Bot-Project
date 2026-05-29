from fastapi import FastAPI
import lightgbm as lgb
import numpy as np
import pandas as pd
import uvicorn

app = FastAPI()

print("🌲 Training LightGBM model...")

# Генерируем обучающие данные
np.random.seed(42)
n_samples = 10000

features = pd.DataFrame({
    'price': 300 + np.random.randn(n_samples) * 20,
    'atr': np.abs(np.random.randn(n_samples) * 3),
    'trend': np.random.choice([-1, 0, 1], n_samples),
    'pressure': np.random.randn(n_samples) * 2,
    'sentiment': np.random.randn(n_samples) * 0.5,
    'volume_ratio': np.random.rand(n_samples),
})

# Целевая переменная (используем ту же логику, что и в байесовской модели)
logits = -0.5 - 0.6 * features['pressure'] + 0.4 * features['trend'] + 0.3 * features['sentiment']
probs = 1 / (1 + np.exp(-logits))
target = (np.random.rand(n_samples) < probs).astype(int)

# Обучаем LightGBM
params = {
    'objective': 'binary',
    'metric': 'logloss',
    'num_leaves': 31,
    'learning_rate': 0.05,
    'feature_fraction': 0.8,
    'bagging_fraction': 0.8,
    'reg_alpha': 0.1,
    'reg_lambda': 0.1,
    'verbose': -1
}

train_data = lgb.Dataset(features, label=target)
model = lgb.train(params, train_data, num_boost_round=100)

print("✅ LightGBM model trained")
print(f"Feature importance: {dict(zip(features.columns, model.feature_importance()))}")

@app.get("/predict")
async def predict(price: float, atr: float, trend: int, pressure: float, sentiment: float = 0.0):
    features = pd.DataFrame([{
        'price': price,
        'atr': atr,
        'trend': trend,
        'pressure': pressure,
        'sentiment': sentiment,
        'volume_ratio': 0.5
    }])
    
    prob = model.predict(features)[0]
    signal = (prob - 0.5) * 2
    
    return {
        "probability": float(prob),
        "signal": float(signal)
    }

@app.get("/health")
async def health():
    return {"status": "ok"}

if __name__ == "__main__":
    print("🚀 LightGBM server on http://localhost:8003")
    uvicorn.run(app, host="0.0.0.0", port=8003)