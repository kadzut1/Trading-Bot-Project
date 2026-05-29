from fastapi import FastAPI
import numpy as np
import pandas as pd
import jax
import jax.numpy as jnp
import numpyro
import numpyro.distributions as dist
from numpyro.infer import MCMC, NUTS
import uvicorn

# Настройка для CPU (чтобы работали 4 цепи параллельно)
numpyro.set_host_device_count(4)

app = FastAPI()

model_samples = None
feature_names = ['price_norm', 'atr_norm', 'trend', 'pressure', 'sentiment']

def load_historical_data():
    print(" Loading historical data...")
    np.random.seed(42)
    n_samples = 5000
    
    data = pd.DataFrame({
        'price_norm': np.random.randn(n_samples),
        'atr_norm': np.abs(np.random.randn(n_samples)),
        'trend': np.random.choice([-1, 0, 1], n_samples),
        'pressure': np.random.randn(n_samples) * 1.5,
        'sentiment': np.random.randn(n_samples) * 0.5,
    })
    
    logits = -0.5 - 0.6 * data['pressure'] + 0.4 * data['trend'] + 0.3 * data['sentiment'] + 0.1 * data['price_norm']
    probs = 1 / (1 + np.exp(-logits))
    data['target'] = (np.random.rand(n_samples) < probs).astype(int)
    
    print(f" Loaded {n_samples} samples. Target balance: {data['target'].mean():.2f}")
    return data

def bayesian_model(X, y=None):
    beta_mean = jnp.array([0.0, 0.0, 0.3, -0.5, 0.2])
    beta_sd = jnp.array([0.5, 0.5, 0.3, 0.3, 0.3])
    beta = numpyro.sample('beta', dist.Normal(beta_mean, beta_sd))
    alpha = numpyro.sample('alpha', dist.Normal(0, 1))
    logits = alpha + jnp.dot(X, beta)
    with numpyro.plate('data', X.shape[0]):
        numpyro.sample('obs', dist.Bernoulli(logits=logits), obs=y)

def train_model():
    global model_samples
    print(" Training Bayesian regression model...")
    
    data = load_historical_data()
    
    X = jnp.array(data[feature_names].values)
    y = jnp.array(data['target'].values)
    
    nuts_kernel = NUTS(bayesian_model)
    mcmc = MCMC(nuts_kernel, num_warmup=500, num_samples=1000, num_chains=4)
    mcmc.run(jax.random.PRNGKey(0), X, y)
    
    model_samples = mcmc.get_samples()
    
    beta_mean = model_samples['beta'].mean(axis=0)
    beta_std = model_samples['beta'].std(axis=0)
    
    print("\n Model coefficients:")
    for name, mean, std in zip(feature_names, beta_mean, beta_std):
        print(f"   {name}: {mean:.3f} ± {std:.3f}")
    
    print("\n Model trained successfully")
    return model_samples

def predict_with_uncertainty(features):
    global model_samples
    if model_samples is None:
        return 0.5, 0.5, 0.0
    
    betas = model_samples['beta']
    alphas = model_samples['alpha']
    
    logits = alphas[:, None] + jnp.dot(features, betas.T)
    probs = 1 / (1 + jnp.exp(-logits))
    
    prob_mean = float(jnp.mean(probs))
    prob_std = float(jnp.std(probs))
    signal = (prob_mean - 0.5) * 2
    
    return prob_mean, prob_std, signal

@app.get("/probability")
async def get_probability(price: float, atr: float, trend: int, pressure: float, sentiment: float = 0.0):
    features = jnp.array([(price - 300) / 50, atr / 5.0, float(trend), pressure, sentiment])
    prob_mean, prob_std, signal = predict_with_uncertainty(features)
    confidence = 1.0 - min(0.5, prob_std * 2)
    adjusted_signal = signal * confidence
    
    return {
        "probability": float(prob_mean),
        "signal": float(adjusted_signal),
        "uncertainty": float(prob_std),
        "confidence": float(confidence),
        "raw_signal": float(signal)
    }

@app.get("/health")
async def health():
    return {"status": "ok", "model_ready": model_samples is not None}

if __name__ == "__main__":
    train_model()
    print("\n Bayesian regression server on http://localhost:8002")
    uvicorn.run(app, host="0.0.0.0", port=8002)