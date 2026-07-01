def trade_balance(exports: float, imports: float) -> float:
    return exports - imports

def terms_of_trade(export_index: float, import_index: float) -> float:
    return 0.0 if import_index == 0.0 else (export_index / import_index) * 100.0

def tariff_revenue(import_value: float, rate: float) -> float:
    return import_value * rate

def revealed_comparative_advantage(country_share: float, world_share: float) -> float:
    return 0.0 if world_share == 0.0 else country_share / world_share
