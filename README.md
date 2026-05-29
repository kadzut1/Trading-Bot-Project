# Trading Bot — Трехфакторный торговый робот для T-Invest API

## 📋 Оглавление
- [Описание проекта](#описание-проекта)
- [Архитектура](#архитектура)
- [Требования к системе](#требования-к-системе)
- [Установка и настройка](#установка-и-настройка)
- [Запуск проекта](#запуск-проекта)
- [Проверка работоспособности](#проверка-работоспособности)
- [Мониторинг (Grafana + Prometheus)](#мониторинг-grafana--prometheus)
- [Управление ботом](#управление-ботом)
- [Устранение неполадок](#устранение-неполадок)
- [Структура проекта](#структура-проекта)

---

## Описание проекта

**Трехфакторный торговый робот** — это автоматизированная система для торговли на фондовом рынке через T-Invest API. Робот принимает решения на основе трех факторов:

| Фактор | Источник | Коэффициент |
|--------|----------|-------------|
| **Временные ряды** | Стакан (DOM depth) + свечи | Главный фильтр |
| **Новостной сентимент** | DeBERTa (LLM) | 0.6 |
| **Математика вероятностей** | Байесовская регрессия + LightGBM | 0.4 |

**Горизонт удержания:** от 2 часов до 3 дней (свинг-трейдинг)

**Технологический стек:**
- **Rust** — высокопроизводительное ядро (fixed-point арифметика, WebSocket, риск-менеджмент)
- **Python** — ML слой (DeBERTa, NumPyro, LightGBM)
- **Redis** — брокер сообщений и оперативное хранение
- **ClickHouse** — историческое хранение данных
- **Prometheus + Grafana** — мониторинг и визуализация
- **Docker** — контейнеризация инфраструктуры

---

## Архитектура

```
┌─────────────────────────────────────────────────────────────────┐
│                        RUST (High-Performance Core)             │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐          │
│  │ WebSocket    │  │ Order Mgmt   │  │ Risk Engine  │          │
│  │ (T-Invest)   │  │ (Rust FFI)   │  │ (Fixed-point)│          │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘          │
│         │                 │                 │                   │
│         └─────────────────┼─────────────────┘                   │
│                           │                                      │
│                    ┌──────▼──────┐                              │
│                    │   Redis     │  (Shared memory / msg bus)   │
│                    └──────┬──────┘                              │
└───────────────────────────┼──────────────────────────────────────┘
                            │
┌───────────────────────────▼──────────────────────────────────────┐
│                     PYTHON (Strategy & ML Layer)                 │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐          │
│  │ News         │  │ Feature      │  │ ML Models    │          │
│  │ Parser       │→ │ Engineering  │→ │ (LightGBM +  │          │
│  │ (DeBERTa)    │  │ (Polars)     │  │  Bayes)      │          │
│  └──────────────┘  └──────────────┘  └──────────────┘          │
└──────────────────────────────────────────────────────────────────┘
```

---

## Требования к системе

### Обязательное ПО

| Компонент | Версия | Ссылка для скачивания |
|-----------|--------|----------------------|
| **Rust** | 1.70+ | https://rustup.rs/ |
| **Python** | 3.12+ | https://www.python.org/downloads/ |
| **Docker Desktop** | 4.20+ | https://www.docker.com/products/docker-desktop/ |
| **Git** | 2.40+ | https://git-scm.com/downloads |

### Проверка установки

```powershell
# Проверка Rust
rustc --version
cargo --version

# Проверка Python
python --version

# Проверка Docker
docker --version
docker-compose --version

# Проверка Git
git --version
```

### Ресурсы системы

| Ресурс | Минимум | Рекомендуется |
|--------|---------|---------------|
| ОЗУ | 8 GB | 16 GB |
| CPU | 4 ядра | 8 ядер |
| Диск | 20 GB | 50 GB (SSD) |
| ОС | Windows 10/11, macOS, Linux | Windows 11 / Ubuntu 22.04 |

---

## Установка и настройка

### Шаг 1. Клонирование репозитория

```powershell
git clone https://github.com/your-repo/trading_bot.git
cd trading_bot
```

### Шаг 2. Настройка переменных окружения

Создай файл `.env` в корневой папке:

```env
TINVEST_TOKEN=твой_токен_из_песочницы
```

**Как получить токен:**
1. Зарегистрируйся в [T-Invest OpenAPI](https://www.tbank.ru/invest/)
2. Перейди в раздел **"Токены"**
3. Создай **Sandbox token** (для тестирования)
4. Скопируй токен в `.env` файл

### Шаг 3. Установка Rust зависимостей

```powershell
cd trading_bot_rust
cargo build
```

### Шаг 4. Установка Python зависимостей

```powershell
cd ../news_sentiment
python -m venv venv
.\venv\Scripts\activate   # Windows
# source venv/bin/activate  # Linux/Mac

pip install -r requirements.txt
```

**Файл `requirements.txt`:**

```txt
fastapi==0.136.0
uvicorn==0.34.0
transformers==4.35.0
torch==2.1.0
lightgbm==4.1.0
numpyro==0.13.0
redis==5.0.0
clickhouse-driver==0.2.6
prometheus-client==0.19.0
requests==2.31.0
python-dotenv==1.0.0
```

### Шаг 5. Запуск инфраструктуры (Docker)

```powershell
docker run -d --name trading-redis -p 6379:6379 redis:7-alpine
docker run -d --name trading-clickhouse -p 8123:8123 -p 9000:9000 clickhouse/clickhouse-server:latest
```

**Проверка:**

```powershell
docker ps
```

---

## Запуск проекта

### Терминал 1: Сервер новостного сентимента (DeBERTa)

```powershell
cd news_sentiment
.\venv\Scripts\activate
python sentiment_server.py
```

Ожидаемый вывод:
```
Sentiment server on http://localhost:8001
INFO:     Started server process
```

### Терминал 2: Сервер байесовской регрессии

```powershell
cd news_sentiment
.\venv\Scripts\activate
python bayes_server.py
```

Ожидаемый вывод:
```
Bayesian server on http://localhost:8002
INFO:     Started server process
```

### Терминал 3: Сервер LightGBM

```powershell
cd news_sentiment
.\venv\Scripts\activate
python lgb_server.py
```

Ожидаемый вывод:
```
LightGBM server on http://localhost:8003
INFO:     Started server process
```

### Терминал 4: Rust бот

```powershell
cd trading_bot_rust
$env:TINVEST_TOKEN="твой_токен"
cargo run --bin three_factor_bot
```

Ожидаемый вывод:
```
🚀 T-Invest Three Factor Bot Started
📈 Instrument: BBG004730N88
📊 Metrics server running on http://localhost:8080/metrics
📡 Signal published: WAIT
```

---

## Мониторинг (Grafana + Prometheus)

### Запуск Prometheus

```powershell
docker run -d --name trading-prometheus -p 9090:9090 prom/prometheus
```

### Запуск Grafana

```powershell
docker run -d --name trading-grafana -p 3001:3000 grafana/grafana
```

### Настройка Grafana

1. Открой `http://localhost:3001`
2. Логин: `admin`, пароль: `admin`
3. **Configuration → Data Sources → Add data source → Prometheus**
4. **URL**: `http://host.docker.internal:9090`
5. **Save & Test**

### Импорт дашборда

1. **Dashboards → Import**
2. Скопируй JSON из `dashboard.json`
3. Нажми **Load → Import**

---

## Проверка работоспособности

### Проверка метрик Rust бота

```powershell
curl http://localhost:8080/metrics
```

Ожидаемый вывод:
```
# HELP current_position_size Current number of shares held
# TYPE current_position_size gauge
current_position_size 0
# HELP portfolio_value Current portfolio value in RUB
# TYPE portfolio_value gauge
portfolio_value 100000
```

### Проверка Prometheus

```powershell
curl "http://localhost:9090/api/v1/query?query=portfolio_value"
```

### Проверка всех сервисов

| Сервис | URL | Ожидаемый статус |
|--------|-----|------------------|
| Rust бот | http://localhost:8080/metrics | 200 OK |
| DeBERTa | http://localhost:8001/sentiment | 200 OK |
| Bayesian | http://localhost:8002/probability | 200 OK |
| LightGBM | http://localhost:8003/predict | 200 OK |
| Prometheus | http://localhost:9090 | 200 OK |
| Grafana | http://localhost:3001 | 200 OK |
| Redis | `docker exec -it trading-redis redis-cli ping` | PONG |
| ClickHouse | `docker exec -it trading-clickhouse clickhouse-client --query "SELECT 1"` | 1 |

---

## Управление ботом

### Проверка баланса в песочнице

```powershell
$token = "твой_токен"
$accountId = "6baab97c-8426-42fe-884c-21d877705b27"

$body = @{accountId = $accountId} | ConvertTo-Json
Invoke-RestMethod -Uri "https://sandbox-invest-public-api.tbank.ru/rest/tinkoff.public.invest.api.contract.v1.OperationsService/GetPositions" -Method Post -Headers @{
    "Authorization" = "Bearer $token"
    "Content-Type" = "application/json"
} -Body $body | Select-Object -ExpandProperty money
```

### Ручная покупка акций

```powershell
$body = @{
    figi = "BBG004730N88"
    quantity = 100
    direction = "ORDER_DIRECTION_BUY"
    accountId = $accountId
    orderType = "ORDER_TYPE_MARKET"
} | ConvertTo-Json

Invoke-RestMethod -Uri "https://sandbox-invest-public-api.tbank.ru/rest/tinkoff.public.invest.api.contract.v1.OrdersService/PostOrder" -Method Post -Headers @{
    "Authorization" = "Bearer $token"
    "Content-Type" = "application/json"
} -Body $body
```

### Ручная продажа акций

```powershell
$body = @{
    figi = "BBG004730N88"
    quantity = 100
    direction = "ORDER_DIRECTION_SELL"
    accountId = $accountId
    orderType = "ORDER_TYPE_MARKET"
} | ConvertTo-Json

Invoke-RestMethod -Uri "https://sandbox-invest-public-api.tbank.ru/rest/tinkoff.public.invest.api.contract.v1.OrdersService/PostOrder" -Method Post -Headers @{
    "Authorization" = "Bearer $token"
    "Content-Type" = "application/json"
} -Body $body
```

### Очистка состояния Redis

```powershell
docker exec -it trading-redis redis-cli FLUSHALL
```

---

## Устранение неполадок

### Ошибка: `TINVEST_TOKEN not set`

**Решение:** Установи переменную окружения

```powershell
$env:TINVEST_TOKEN="твой_токен"
```

### Ошибка: `Connection refused` (порт 8080)

**Решение:** Rust бот не запущен. Запусти:

```powershell
cargo run --bin three_factor_bot
```

### Ошибка: `invalid peer certificate: UnknownIssuer`

**Решение:** Отключи проверку SSL в Python скриптах (добавь `verify=False`)

### Ошибка: `Not enough balance`

**Решение:** Пополни баланс в песочнице

```powershell
$body = @{
    accountId = $accountId
    amount = @{units = 100000; nano = 0}
} | ConvertTo-Json

Invoke-RestMethod -Uri "https://sandbox-invest-public-api.tbank.ru/rest/tinkoff.public.invest.api.contract.v1.SandboxService/SandboxPayIn" -Method Post -Headers @{
    "Authorization" = "Bearer $token"
    "Content-Type" = "application/json"
} -Body $body
```

### Ошибка: `docker: command not found`

**Решение:** Установи Docker Desktop с официального сайта

### Ошибка: `cargo: command not found`

**Решение:** Установи Rust: https://rustup.rs/

### Grafana не видит Prometheus

**Решение:** Проверь URL источника данных

```yaml
URL: http://host.docker.internal:9090
```

Или используй:

```yaml
URL: http://localhost:9090
```

---

## Структура проекта

```
trading_bot/
├── trading_bot_rust/           # Rust Core
│   ├── src/
│   │   ├── bin/
│   │   │   └── three_factor_bot.rs   # Основной бот
│   │   ├── domain/                    # Модели данных
│   │   │   ├── price.rs              # Fixed-point Price(i128)
│   │   │   ├── candle.rs             # Свечи
│   │   │   ├── order_book.rs         # Стакан
│   │   │   └── analyzer.rs           # ATR, RSI, тренды
│   │   ├── redis_client.rs           # Redis клиент
│   │   └── state_manager.rs          # Сохранение состояния
│   ├── Cargo.toml
│   └── .env
├── news_sentiment/             # Python ML слой
│   ├── sentiment_server.py     # DeBERTa сервер
│   ├── bayes_server.py         # NumPyro байесовская регрессия
│   ├── lgb_server.py           # LightGBM
│   ├── redis_listener.py       # Подписка на сигналы
│   ├── requirements.txt
│   └── venv/
└── docker-compose.yml          # Инфраструктура (Redis, ClickHouse)
```

---

## Контакты и поддержка

При возникновении вопросов:

1. Проверь [T-Invest API документацию](https://developer.tbank.ru/invest/intro/intro)
2. Проверь [лог-файлы](#устранение-неполадок)
3. Открой Issue в репозитории проекта

---

**© 2026 Trading Bot Project**