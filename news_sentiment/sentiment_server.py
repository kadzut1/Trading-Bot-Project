from fastapi import FastAPI
from transformers import AutoTokenizer, AutoModelForSequenceClassification
import torch
import uvicorn
import threading
import time
import datetime
from news_fetcher import NewsFetcher, NewsItem

app = FastAPI()

# Загружаем модель
MODEL_NAME = "cointegrated/rubert-tiny2"
tokenizer = AutoTokenizer.from_pretrained(MODEL_NAME)
model = AutoModelForSequenceClassification.from_pretrained(MODEL_NAME, num_labels=3)

# Инициализируем парсер новостей
news_fetcher = NewsFetcher()
FIGI = "BBG004730N88"

# Финансовые ключевые слова
POSITIVE_KEYWORDS = [
    'рекорд', 'прибыль', 'рост', 'выше', 'хорошо', 'успех', 'дивиденды',
    'позитивный', 'увеличил', 'повысил', 'лучше', 'доход', 'акции растут'
]

NEGATIVE_KEYWORDS = [
    'убыток', 'падение', 'ниже', 'проблемы', 'риск', 'санкции', 'кризис',
    'снижение', 'потеря', 'хуже', 'негативный'
]

# Кэш для последнего агрегированного сентимента
cached_sentiment = {
    "overall": 0.0,
    "count": 0,
    "positive": 0,
    "negative": 0,
    "updated_at": None,
    "news": []
}

def analyze_sentiment(text: str):
    """Анализ тональности с учётом финансовых ключевых слов"""
    text_lower = text.lower()
    
    # Анализ ключевых слов
    pos_count = sum(1 for kw in POSITIVE_KEYWORDS if kw in text_lower)
    neg_count = sum(1 for kw in NEGATIVE_KEYWORDS if kw in text_lower)
    
    keyword_score = 0.0
    if pos_count > neg_count:
        keyword_score = min(0.8, 0.3 + pos_count * 0.1)
    elif neg_count > pos_count:
        keyword_score = -min(0.8, 0.3 + neg_count * 0.1)
    
    # Если нет ключевых слов, используем модель
    if keyword_score == 0.0:
        inputs = tokenizer(text[:512], return_tensors="pt", truncation=True, max_length=512)
        with torch.no_grad():
            outputs = model(**inputs)
        probs = torch.softmax(outputs.logits, dim=-1).numpy()[0]
        # negative, neutral, positive
        model_score = probs[2] - probs[0]
        final_score = model_score
    else:
        final_score = keyword_score
    
    final_score = max(-0.95, min(0.95, final_score))
    
    if final_score > 0.3:
        return final_score, "positive"
    elif final_score < -0.3:
        return final_score, "negative"
    else:
        return 0.0, "neutral"

def process_news(news):
    """Callback для обработки новой новости"""
    score, label = analyze_sentiment(news.title + " " + news.content)
    news.sentiment_score = score
    news.sentiment_label = label
    update_aggregated_sentiment()
    print(f" NEWS: {news.title[:80]}... ({label}: {score:.2f})")
    return news

def update_aggregated_sentiment():
    """Обновляет кэш агрегированного сентимента"""
    global cached_sentiment
    agg = news_fetcher.aggregate_sentiment(FIGI, hours=4)
    cached_sentiment = {
        "overall": agg["sentiment"],
        "count": agg["count"],
        "positive": agg["positive_count"],
        "negative": agg["negative_count"],
        "updated_at": datetime.datetime.now().isoformat(),
        "news": agg["news"]
    }

@app.get("/sentiment")
async def get_sentiment():
    """Возвращает агрегированный сентимент по новостям за 4 часа"""
    return {
        "sentiment": "positive" if cached_sentiment["overall"] > 0.2 else "negative" if cached_sentiment["overall"] < -0.2 else "neutral",
        "score": abs(cached_sentiment["overall"]),
        "news_count": cached_sentiment["count"],
        "positive_count": cached_sentiment["positive"],
        "negative_count": cached_sentiment["negative"],
        "updated_at": cached_sentiment.get("updated_at")
    }

@app.on_event("startup")
async def startup_event():
    """Запускаем фоновый сбор новостей при старте"""
    print(" Loading news...")
    
    # Загружаем тестовые новости
    test_news = news_fetcher.fetch_news()
    
    # Анализируем каждую новость
    for news in test_news:
        score, label = analyze_sentiment(news.title + " " + news.content)
        news.sentiment_score = score
        news.sentiment_label = label
        print(f"   📰 {news.title[:60]}... ({label}: {score:.2f})")
    
    # Обновляем агрегированный сентимент
    update_aggregated_sentiment()
    
    print(f" Initial sentiment: {cached_sentiment['overall']:.2f} (based on {cached_sentiment['count']} news)")
    print(f" Sentiment server started on http://localhost:8001")

if __name__ == "__main__":
    print("Sentiment server with news parser on http://localhost:8001")
    uvicorn.run(app, host="0.0.0.0", port=8001)