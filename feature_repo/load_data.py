import pandas as pd
import numpy as np
from datetime import datetime, timedelta
import os

os.makedirs('data', exist_ok=True)

figi = 'BBG004730N88'

# Candles
dates = pd.date_range(start='2024-01-01', end='2025-01-01', freq='1h')
candles = []
for ts in dates[:500]:
    price = 320 + np.random.randn() * 5
    candles.append({
        'figi': figi,
        'timestamp': ts,
        'open': price,
        'high': price + abs(np.random.randn() * 2),
        'low': price - abs(np.random.randn() * 2),
        'close': price + np.random.randn(),
        'volume': int(np.random.rand() * 1000000),
        'atr_14': abs(np.random.randn() * 3),
    })
pd.DataFrame(candles).to_parquet('data/candles.parquet')

# Orderbook
orderbook = []
for i in range(500):
    orderbook.append({
        'figi': figi,
        'timestamp': datetime.now() - timedelta(minutes=i),
        'pressure': np.random.randn() * 2,
        'bid_volume_ratio': np.random.rand(),
        'ask_volume_ratio': np.random.rand(),
    })
pd.DataFrame(orderbook).to_parquet('data/orderbook.parquet')

# Sentiment
sentiment = []
for i in range(500):
    sentiment.append({
        'figi': figi,
        'timestamp': datetime.now() - timedelta(hours=i),
        'news_sentiment': np.random.randn() * 0.5,
        'positive_count': np.random.randint(0, 5),
        'negative_count': np.random.randint(0, 5),
    })
pd.DataFrame(sentiment).to_parquet('data/sentiment.parquet')

print('Data created')