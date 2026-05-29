import pandas as pd
import numpy as np
from datetime import datetime, timedelta
from typing import Dict, List, Tuple
import lightgbm as lgb

class WalkForwardValidator:
    def __init__(self, figi: str = "BBG004730N88"):
        self.figi = figi
        self.results = []
        
    def generate_synthetic_data(self, start_date: datetime, end_date: datetime, 
                                 seed: int = None) -> pd.DataFrame:
        """Генерирует реалистичные синтетические данные"""
        if seed:
            np.random.seed(seed)
        
        dates = pd.date_range(start=start_date, end=end_date, freq='1h')
        n = len(dates)
        
        # Цена с трендом, циклом и шумом
        trend = np.linspace(0, 0.15, n)  # 15% восходящий тренд за период
        cycle = 0.08 * np.sin(2 * np.pi * np.arange(n) / (24 * 21))  # 21-дневный цикл
        noise = np.random.randn(n) * 0.025
        returns = trend + cycle + noise
        price = 300 * np.exp(np.cumsum(returns))
        
        # Давление (коррелирует с доходностью)
        pressure = -np.sign(returns) * np.random.rand(n) * 1.5
        
        # Сентимент (коррелирует с предыдущей доходностью)
        returns_series = pd.Series(returns)
        sentiment = returns_series.shift(1).fillna(0) * 0.5 + np.random.randn(n) * 0.3
        sentiment = sentiment.clip(-1, 1).values
        
        data = []
        for i, (dt, p) in enumerate(zip(dates, price)):
            volatility = 0.015 * (1 + abs(returns[i]) * 5)
            high = p * (1 + np.random.rand() * volatility)
            low = p * (1 - np.random.rand() * volatility)
            open_price = low + (high - low) * np.random.rand()
            close_price = low + (high - low) * np.random.rand()
            volume = int(np.random.rand() * 1000000)
            
            data.append({
                "timestamp": dt,
                "open": open_price,
                "high": high,
                "low": low,
                "close": close_price,
                "volume": volume,
                "pressure": pressure[i],
                "sentiment": sentiment[i]
            })
        
        return pd.DataFrame(data)
    
    def calculate_features(self, df: pd.DataFrame) -> pd.DataFrame:
        """Рассчитывает признаки"""
        data = df.copy()
        
        # Доходности
        data['returns'] = data['close'].pct_change()
        data['returns_5'] = data['close'].pct_change(5)
        data['returns_10'] = data['close'].pct_change(10)
        data['returns_20'] = data['close'].pct_change(20)
        
        # ATR
        data['tr'] = np.maximum(
            data['high'] - data['low'],
            np.maximum(
                abs(data['high'] - data['close'].shift(1)),
                abs(data['low'] - data['close'].shift(1))
            )
        )
        data['atr_14'] = data['tr'].rolling(14).mean()
        
        # RSI
        delta = data['close'].diff()
        gain = delta.where(delta > 0, 0).rolling(14).mean()
        loss = (-delta.where(delta < 0, 0)).rolling(14).mean()
        data['rsi_14'] = 100 - (100 / (1 + gain / loss))
        
        # Признаки из стакана и новостей
        data['pressure_norm'] = data['pressure'] / 2.0
        data['sentiment_norm'] = data['sentiment']
        
        # Цель: цена вырастет через 4 часа
        data['target'] = (data['close'].shift(-4) > data['close'] * 1.005).astype(int)
        
        data = data.dropna()
        return data
    
    def train_lightgbm(self, train_df: pd.DataFrame) -> lgb.Booster:
        """Обучает LightGBM"""
        feature_cols = ['returns', 'returns_5', 'returns_10', 'returns_20', 
                       'atr_14', 'rsi_14', 'pressure_norm', 'sentiment_norm']
        
        X_train = train_df[feature_cols]
        y_train = train_df['target']
        
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
        
        train_data = lgb.Dataset(X_train, label=y_train)
        model = lgb.train(params, train_data, num_boost_round=100)
        return model
    
    def evaluate_model(self, model: lgb.Booster, test_df: pd.DataFrame) -> Dict:
        """Оценивает модель"""
        feature_cols = ['returns', 'returns_5', 'returns_10', 'returns_20', 
                       'atr_14', 'rsi_14', 'pressure_norm', 'sentiment_norm']
        
        X_test = test_df[feature_cols]
        y_test = test_df['target']
        
        predictions = model.predict(X_test)
        predictions_binary = (predictions > 0.5).astype(int)
        
        accuracy = (predictions_binary == y_test).mean()
        
        test_copy = test_df.copy()
        test_copy['prediction'] = predictions_binary
        test_copy['signal_return'] = test_copy['returns'] * test_copy['prediction']
        
        total_return = test_copy['signal_return'].sum()
        sharpe = test_copy['signal_return'].mean() / max(0.01, test_copy['signal_return'].std())
        
        cumulative = (1 + test_copy['signal_return']).cumprod()
        running_max = cumulative.expanding().max()
        drawdown = (cumulative - running_max) / running_max
        max_drawdown = drawdown.min()
        
        return {
            "accuracy": accuracy,
            "total_return": total_return,
            "sharpe": sharpe,
            "max_drawdown": max_drawdown,
            "n_trades": predictions_binary.sum()
        }
    
    def run_walk_forward(self, train_months: int = 3, test_months: int = 1,
                         purge_days: int = 14, n_windows: int = 6):
        """Запускает Walk-Forward валидацию"""
        
        print("=" * 80)
        print("WALK-FORWARD VALIDATION (Synthetic Data)")
        print("=" * 80)
        print(f"Train period: {train_months} months")
        print(f"Test period: {test_months} months")
        print(f"Purge gap: {purge_days} days")
        print(f"Windows: {n_windows}")
        print("=" * 80)
        
        results = []
        
        for window in range(1, n_windows + 1):
            # Генерируем непересекающиеся окна
            offset = (window - 1) * (train_months + test_months) + purge_days / 30
            
            train_start = datetime(2020, 1, 1) + timedelta(days=30 * offset)
            train_end = train_start + timedelta(days=30 * train_months)
            test_start = train_end + timedelta(days=purge_days)
            test_end = test_start + timedelta(days=30 * test_months)
            
            print(f"\n📊 Window {window}")
            print(f"   Train: {train_start.date()} → {train_end.date()}")
            print(f"   Test:  {test_start.date()} → {test_end.date()}")
            
            # Генерируем данные
            train_data = self.generate_synthetic_data(train_start, train_end, seed=window)
            test_data = self.generate_synthetic_data(test_start, test_end, seed=window + 100)
            
            # Рассчитываем признаки
            train_df = self.calculate_features(train_data)
            test_df = self.calculate_features(test_data)
            
            if len(train_df) < 50 or len(test_df) < 20:
                print("   ⚠️ Insufficient data, skipping")
                continue
            
            # Обучаем
            print("   Training LightGBM...")
            model = self.train_lightgbm(train_df)
            
            # Оцениваем
            metrics = self.evaluate_model(model, test_df)
            metrics["window"] = window
            
            results.append(metrics)
            
            print(f"   ✅ Accuracy: {metrics['accuracy']:.2%}")
            print(f"   📈 Return: {metrics['total_return']:.2%}")
            print(f"   📊 Sharpe: {metrics['sharpe']:.2f}")
            print(f"   📉 Max DD: {metrics['max_drawdown']:.2%}")
        
        # Сводка
        self.results = results
        self.print_summary()
        
        return results
    
    def print_summary(self):
        print("\n" + "=" * 80)
        print("WALK-FORWARD SUMMARY")
        print("=" * 80)
        
        if not self.results:
            print("No results")
            return
        
        df = pd.DataFrame(self.results)
        
        avg_acc = df['accuracy'].mean()
        win_rate = (df['total_return'] > 0).mean()
        avg_ret = df['total_return'].mean()
        avg_sharpe = df['sharpe'].mean()
        avg_dd = df['max_drawdown'].mean()
        
        print(f"📊 Windows: {len(self.results)}")
        print(f"🎯 Avg Accuracy: {avg_acc:.2%}")
        print(f"✅ Profitable windows: {win_rate:.0%} (need >60%)")
        print(f"📈 Avg Return: {avg_ret:.2%}")
        print(f"📊 Avg Sharpe: {avg_sharpe:.2f}")
        print(f"📉 Avg Max DD: {avg_dd:.2%}")
        
        if win_rate > 0.6:
            print("\n✅ MAJORITY PASS RULE: PASSED")
        else:
            print(f"\n❌ MAJORITY PASS RULE: FAILED (only {win_rate:.0%} profitable)")
        
        min_ret = df['total_return'].min()
        if min_ret < -0.2:
            print(f"❌ CATASTROPHIC VETO: FAILED (worst: {min_ret:.2%} < -20%)")
        else:
            print(f"✅ CATASTROPHIC VETO: PASSED (worst: {min_ret:.2%})")
        
        ret_std = df['total_return'].std()
        if ret_std < 0.01:
            print("❌ CLIFF VETO: FAILED (returns are flat)")
        else:
            print(f"✅ CLIFF VETO: PASSED (std: {ret_std:.4f})")
        
        print("\n" + "=" * 80)
        
        if win_rate > 0.6 and min_ret > -0.2 and ret_std >= 0.01:
            print("🏆 VERDICT: MODEL VALID - Ready for trading")
        else:
            print("⚠️ VERDICT: MODEL NEEDS IMPROVEMENT")

if __name__ == "__main__":
    validator = WalkForwardValidator()
    results = validator.run_walk_forward(n_windows=6)