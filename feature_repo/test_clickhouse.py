from clickhouse_driver import Client

client = Client(
    host='localhost',
    port=9000,
    user='default',
    password='clickhouse123'
)

client.execute('SELECT 1')
print("Connected!")

# Создаём таблицу
client.execute('''
    CREATE TABLE IF NOT EXISTS candles (
        figi String,
        timestamp DateTime,
        open Float64,
        high Float64,
        low Float64,
        close Float64,
        volume UInt64
    ) ENGINE = MergeTree()
    ORDER BY timestamp
''')

print("Table ready")