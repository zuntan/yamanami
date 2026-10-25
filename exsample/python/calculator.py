"""Arithmetic is deliberately independent of the WASI HTTP bindings."""
import math
import re

NUMBER = re.compile(r"^[+-]?(?:[0-9]+(?:\.[0-9]*)?|\.[0-9]+)(?:[eE][+-]?[0-9]+)?$")


def calculate(a: str, op: str, b: str) -> float:
    if len(a) > 100 or len(b) > 100 or not NUMBER.fullmatch(a) or not NUMBER.fullmatch(b):
        raise ValueError("数値を入力してください")
    left, right = float(a), float(b)
    if not math.isfinite(left) or not math.isfinite(right):
        raise ValueError("有限の数値を入力してください")
    if op == "add":
        result = left + right
    elif op == "sub":
        result = left - right
    elif op == "mul":
        result = left * right
    elif op == "div":
        if right == 0:
            raise ValueError("ゼロで除算できません")
        result = left / right
    else:
        raise ValueError("不明な演算子です")
    if not math.isfinite(result):
        raise ValueError("結果が数値の範囲を超えています")
    return result
