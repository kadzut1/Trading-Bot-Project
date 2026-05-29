import feedparser
import hashlib
import datetime
import json
import time
import threading
import requests
from typing import List, Dict, Any
from dataclasses import dataclass, asdict

@dataclass
class NewsItem:
    title: str
    link: str
    published: datetime.datetime
    source: str
    content: str
    hash_id: str
    sentiment_score: float = 0.0
    sentiment_label: str = "neutral"

class NewsFetcher:
    """Парсер новостей для финансового сентимента"""
    
    # Финансовые ключевые слова для Сбербанка
    KEYWORDS = {
        "BBG004730N88": ["сбер", "сбербанк", "sber", "sberbank"],
    }
    
    def __init__(self):
        self.seen_hashes = set()
        self.news_history: List[NewsItem] = []
        self.callback = None
        self.running = False
        
    def _compute_hash(self, title: str, link: str) -> str:
        """Вычисляем хеш для дедупликации"""
        content = f"{title}|{link}"
        return hashlib.md5(content.encode()).hexdigest()
    
    def _get_mock_news(self) -> List[dict]:
        """Тестовые новости для Сбербанка (работает всегда)"""
        now = datetime.datetime.now()
        return [
            {
                "title": "Сбербанк показал рекордную прибыль в 2024 году. Чистая прибыль выросла на 35%",
                "content": "Сбербанк опубликовал отчетность по МСФО. Прибыль достигла 1.5 трлн рублей, что является историческим рекордом. Акции компании выросли на 5%.",
                "source": "mock",
                "published": now - datetime.timedelta(hours=1),
                "link": "mock1"
            },
            {
                "title": "Аналитики повысили прогноз по акциям Сбера до 350 рублей",
                "content": "Инвестиционные банки пересмотрели целевые цены по акциям Сбербанка вверх. Рекомендация - покупать.",
                "source": "mock",
                "published": now - datetime.timedelta(hours=2),
                "source": "mock",
                "link": "mock2"
            },
            {
                "title": "Сбербанк увеличил дивиденды для акционеров. Рекордные выплаты",
                "content": "Совет директоров рекомендовал выплатить дивиденды в размере 35 рублей на акцию. Дивидендная доходность составит 11%.",
                "source": "mock",
                "published": now - datetime.timedelta(hours=3),
                "link": "mock3"
            },
            {
                "title": "Сбербанк запустил новую программу кредитования бизнеса",
                "content": "Банк предлагает льготные ставки для малого и среднего бизнеса. Это позитивно скажется на кредитном портфеле.",
                "source": "mock",
                "published": now - datetime.timedelta(hours=4),
                "link": "mock4"
            },
            {
                "title": "Международные инвесторы увеличивают вложения в акции Сбера",
                "content": "Иностранные фонды наращивают долю в капитале Сбербанка. Это сигнал доверия к российской экономике.",
                "source": "mock",
                "published": now - datetime.timedelta(hours=5),
                "link": "mock5"
            }
        ]
    
    def fetch_news(self) -> List[NewsItem]:
        """Загружает новости (использует mock данные)"""
        new_news = []
        
        # Используем тестовые новости
        print(f"📡 Using mock news data...")
        mock_news = self._get_mock_news()
        
        for news_data in mock_news:
            title = news_data.get('title', '')
            if not title:
                continue
            
            link = news_data.get('link', '')
            published = news_data.get('published', datetime.datetime.now())
            content = news_data.get('content', title)
            source = news_data.get('source', 'mock')
            
            hash_id = self._compute_hash(title, link)
            if hash_id in self.seen_hashes:
                continue
            
            self.seen_hashes.add(hash_id)
            
            news_item = NewsItem(
                title=title,
                link=link,
                published=published,
                source=source,
                content=content,
                hash_id=hash_id
            )
            new_news.append(news_item)
            self.news_history.append(news_item)
        
        # Ограничиваем историю
        if len(self.news_history) > 1000:
            self.news_history = self.news_history[-500:]
        
        print(f" Total news in history: {len(self.news_history)}")
        return new_news
    
    def filter_by_figi(self, news_items: List[NewsItem], figi: str) -> List[NewsItem]:
        """Фильтрует новости по ключевым словам для инструмента"""
        keywords = self.KEYWORDS.get(figi, [])
        if not keywords:
            return news_items
        
        filtered = []
        for item in news_items:
            text = (item.title + " " + item.content).lower()
            for kw in keywords:
                if kw.lower() in text:
                    filtered.append(item)
                    break
        return filtered
    
    def aggregate_sentiment(self, figi: str, hours: int = 4) -> Dict[str, Any]:
        """
        Агрегирует сентимент по новостям за последние N часов.
        По ТЗ: окно 4 часа, с весами (свежие важнее)
        """
        cutoff = datetime.datetime.now() - datetime.timedelta(hours=hours)
        
        # Получаем новости за последние часы
        relevant = [
            n for n in self.news_history 
            if n.published > cutoff and n.sentiment_score != 0
        ]
        
        # Фильтруем по FIGI
        relevant = self.filter_by_figi(relevant, figi)
        
        if not relevant:
            return {
                "count": 0,
                "sentiment": 0.0,
                "positive_count": 0,
                "negative_count": 0,
                "news": []
            }
        
        # Агрегация с весами (свежие новости имеют больший вес)
        weighted_sum = 0.0
        total_weight = 0.0
        positive_count = 0
        negative_count = 0
        
        for news in relevant:
            # Время в часах назад
            hours_ago = (datetime.datetime.now() - news.published).total_seconds() / 3600
            # Вес: экспоненциальное затухание (свежее = больше вес)
            weight = 2.0 ** (-hours_ago / 4)  # Полураспад 4 часа
            if hours_ago > 4:
                weight *= 0.5
            
            weighted_sum += news.sentiment_score * weight
            total_weight += weight
            
            if news.sentiment_score > 0.2:
                positive_count += 1
            elif news.sentiment_score < -0.2:
                negative_count += 1
        
        if total_weight > 0:
            final_sentiment = weighted_sum / total_weight
        else:
            final_sentiment = 0.0
        
        # Ограничиваем диапазон [-1, 1]
        final_sentiment = max(-1.0, min(1.0, final_sentiment))
        
        return {
            "count": len(relevant),
            "sentiment": final_sentiment,
            "positive_count": positive_count,
            "negative_count": negative_count,
            "news": [{"title": n.title, "score": n.sentiment_score, "time": n.published.isoformat()} 
                    for n in relevant[-5:]]
        }
    
    def start_background_fetch(self, interval_seconds: int = 300, callback=None):
        """Запускает фоновый сбор новостей"""
        self.running = True
        self.callback = callback
        
        def fetch_loop():
            while self.running:
                try:
                    print(f"\n Fetching news at {datetime.datetime.now()}")
                    new_news = self.fetch_news()
                    
                    if new_news and self.callback:
                        for news in new_news:
                            self.callback(news)
                    
                    print(f" Total news in history: {len(self.news_history)}")
                    
                except Exception as e:
                    print(f" Fetch error: {e}")
                
                time.sleep(interval_seconds)
        
        thread = threading.Thread(target=fetch_loop, daemon=True)
        thread.start()
        print(f" Background news fetcher started (interval: {interval_seconds}s)")

# Тест
if __name__ == "__main__":
    fetcher = NewsFetcher()
    
    print("Testing news fetcher...")
    news = fetcher.fetch_news()
    print(f"Found {len(news)} news items")
    
    for n in news[:5]:
        print(f"  - {n.title[:80]}...")
        print(f"    Source: {n.source}, Time: {n.published}")