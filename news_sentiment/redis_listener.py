import redis
import json
import datetime
import requests
import threading
import time

# Подключение к Redis
r = redis.Redis(host='localhost', port=6379, decode_responses=True)
CHANNEL = "trading.signals"

# URL серверов
SENTIMENT_URL = "http://localhost:8001/sentiment"
BAYESIAN_URL = "http://localhost:8002/probability"

# Конфигурация
FIGI = "BBG004730N88"
NEWS_CHECK_INTERVAL = 300  # 5 минут

# Кэш для агрегированного сентимента
aggregated_sentiment = {
    "value": 0.0,
    "updated_at": None,
    "news_count": 0,
    "positive_count": 0,
    "negative_count": 0
}

def get_aggregated_sentiment_from_server():
    """Получает агрегированный сентимент из новостного сервера (по всем новостям за 4 часа)"""
    try:
        # Без текста — возвращает агрегированный сентимент
        response = requests.get(SENTIMENT_URL, timeout=3)
        if response.status_code == 200:
            data = response.json()
            score = data.get('score', 0.0)
            sentiment = data.get('sentiment', 'neutral')
            news_count = data.get('news_count', 0)
            positive_count = data.get('positive_count', 0)
            negative_count = data.get('negative_count', 0)
            
            return {
                "value": score if sentiment == 'positive' else -score,
                "news_count": news_count,
                "positive_count": positive_count,
                "negative_count": negative_count,
                "updated_at": datetime.datetime.now().isoformat()
            }
    except Exception as e:
        print(f"  ⚠️ Failed to get aggregated sentiment: {e}")
    
    return None

def get_news_sentiment_for_text(text):
    """Вызов DeBERTa для конкретного текста"""
    try:
        response = requests.get(f"{SENTIMENT_URL}?text={text}", timeout=2)
        if response.status_code == 200:
            data = response.json()
            sentiment = data.get('sentiment', 'neutral')
            score = data.get('score', 0.0)
            if sentiment == 'positive':
                return score
            elif sentiment == 'negative':
                return -score
        return 0.0
    except Exception as e:
        print(f"  ⚠️ Sentiment error: {e}")
        return 0.0

def get_bayesian(price, atr, trend, pressure):
    """Вызов байесовского сервера"""
    try:
        resp = requests.get(f"{BAYESIAN_URL}?price={price}&atr={atr}&trend={trend}&pressure={pressure}", timeout=2)
        if resp.status_code == 200:
            return resp.json().get('signal', 0.0)
    except:
        pass
    return 0.0

def refresh_sentiment_cache():
    """Фоновая задача для обновления кэша сентимента"""
    global aggregated_sentiment
    
    while True:
        try:
            new_sentiment = get_aggregated_sentiment_from_server()
            if new_sentiment:
                aggregated_sentiment = new_sentiment
                print(f"📊 Sentiment cache updated: {aggregated_sentiment['value']:.2f} (based on {aggregated_sentiment['news_count']} news)")
            else:
                print(f"⚠️ Could not refresh sentiment cache")
        except Exception as e:
            print(f"⚠️ Cache refresh error: {e}")
        
        time.sleep(NEWS_CHECK_INTERVAL)

def start_cache_refresher():
    """Запускает фоновое обновление кэша сентимента"""
    thread = threading.Thread(target=refresh_sentiment_cache, daemon=True)
    thread.start()
    print(f"🔄 Sentiment cache refresher started (every {NEWS_CHECK_INTERVAL}s)")

def get_current_sentiment():
    """Возвращает актуальный сентимент (из кэша или прямой запрос)"""
    global aggregated_sentiment
    
    # Если кэш свежий (менее 5 минут), используем его
    if aggregated_sentiment["updated_at"]:
        updated = datetime.datetime.fromisoformat(aggregated_sentiment["updated_at"])
        if (datetime.datetime.now() - updated).seconds < NEWS_CHECK_INTERVAL:
            return aggregated_sentiment["value"]
    
    # Иначе делаем прямой запрос
    fresh = get_aggregated_sentiment_from_server()
    if fresh:
        aggregated_sentiment = fresh
        return fresh["value"]
    
    return 0.0

def process_signal(data):
    """Обработка сигнала с реальным сентиментом из новостей"""
    print(f"\n{'='*60}")
    print(f"[{datetime.datetime.now().strftime('%Y-%m-%d %H:%M:%S')}] SIGNAL RECEIVED")
    print(f"  Decision from Rust: {data.get('decision', 'UNKNOWN')}")
    print(f"  Price: {data.get('price', 0):.2f}")
    print(f"  Pressure: {data.get('pressure', 0):.4f}")
    
    # РЕАЛЬНЫЙ сентимент из новостей (агрегированный за 4 часа)
    sentiment = get_current_sentiment()
    print(f"  News Sentiment (4h window): {sentiment:.2f}")
    if aggregated_sentiment.get('news_count', 0) > 0:
        print(f"    News analyzed: {aggregated_sentiment['news_count']} (pos: {aggregated_sentiment['positive_count']}, neg: {aggregated_sentiment['negative_count']})")
    
    # Получаем давление для байесовского фактора
    price = data.get('price', 0)
    pressure = data.get('pressure', 0)
    
    # Байесовский фактор (на основе давления и тренда)
    bayesian = 0.0
    if pressure < -0.5:
        bayesian = 0.6
    elif pressure > 0.5:
        bayesian = -0.6
    else:
        bayesian = 0.3 if pressure < 0 else -0.3
    
    print(f"  Bayesian factor: {bayesian:.2f}")
    
    # Взвешенная сумма по ТЗ (новости 0.6, байес 0.4)
    weighted_sum = sentiment * 0.6 + bayesian * 0.4
    print(f"  Weighted sum: {weighted_sum:.2f}")
    
    # Главный фильтр: давление BULLISH (отрицательное)
    main_filter = pressure < 0
    print(f"  Main filter (pressure < 0): {main_filter}")
    
    rust_decision = data.get('decision', 'WAIT')
    
    # Финальное решение по ТЗ
    if main_filter and weighted_sum > 0.3:
        final_decision = "LONG"
        print(f"\n🔥🔥 FINAL DECISION: LONG ENTRY")
        print(f"   Reason: Main filter TRUE + Weighted sum {weighted_sum:.2f} > 0.3")
        
    elif main_filter and weighted_sum < -0.3:
        final_decision = "SHORT"
        print(f"\n❄️❄️ FINAL DECISION: SHORT ENTRY")
        print(f"   Reason: Main filter TRUE + Weighted sum {weighted_sum:.2f} < -0.3")
        
    else:
        final_decision = "WAIT"
        print(f"\n⏸️ FINAL DECISION: WAIT")
        if not main_filter:
            print(f"   Reason: Main filter FALSE (pressure >= 0)")
        else:
            print(f"   Reason: Weighted sum {weighted_sum:.2f} not exceeding threshold ±0.3")
    
    print('='*60)
    return final_decision

def main():
    print(f"🐍 Trading Bot Listener with Real News Parser")
    print(f"📡 Subscribing to Redis channel: {CHANNEL}")
    print(f"🎯 Instrument: {FIGI}")
    print(f"📰 News sentiment window: 4 hours")
    print("=" * 60)
    
    # Запускаем фоновое обновление кэша сентимента
    start_cache_refresher()
    
    # Подписываемся на Redis
    pubsub = r.pubsub()
    pubsub.subscribe(CHANNEL)
    
    print("✅ Ready and waiting for signals...\n")
    
    for message in pubsub.listen():
        if message['type'] == 'message':
            try:
                data = json.loads(message['data'])
                process_signal(data)
            except json.JSONDecodeError as e:
                print(f"❌ JSON decode error: {e}")
            except Exception as e:
                print(f"❌ Error processing signal: {e}")

if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        print("\n👋 Shutting down...")
    except Exception as e:
        print(f"❌ Fatal error: {e}")