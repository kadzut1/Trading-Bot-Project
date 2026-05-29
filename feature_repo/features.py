from feast import Entity, Feature, FeatureView, FileSource
from feast.types import Float32, Int64
from datetime import timedelta

figi = Entity(name='figi', description='Instrument FIGI')

candles_source = FileSource(
    path='data/candles.parquet',
    event_timestamp_column='timestamp',
)

candles_fv = FeatureView(
    name='candles',
    entities=[figi],
    ttl=timedelta(days=30),
    features=[
        Feature(name='open', dtype=Float32),
        Feature(name='high', dtype=Float32),
        Feature(name='low', dtype=Float32),
        Feature(name='close', dtype=Float32),
        Feature(name='volume', dtype=Int64),
        Feature(name='atr_14', dtype=Float32),
    ],
    batch_source=candles_source,
)

orderbook_source = FileSource(
    path='data/orderbook.parquet',
    event_timestamp_column='timestamp',
)

orderbook_fv = FeatureView(
    name='orderbook',
    entities=[figi],
    ttl=timedelta(days=7),
    features=[
        Feature(name='pressure', dtype=Float32),
        Feature(name='bid_volume_ratio', dtype=Float32),
        Feature(name='ask_volume_ratio', dtype=Float32),
    ],
    batch_source=orderbook_source,
)

sentiment_source = FileSource(
    path='data/sentiment.parquet',
    event_timestamp_column='timestamp',
)

sentiment_fv = FeatureView(
    name='sentiment',
    entities=[figi],
    ttl=timedelta(days=7),
    features=[
        Feature(name='news_sentiment', dtype=Float32),
        Feature(name='positive_count', dtype=Int64),
        Feature(name='negative_count', dtype=Int64),
    ],
    batch_source=sentiment_source,
)