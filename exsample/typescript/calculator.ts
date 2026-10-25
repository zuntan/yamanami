const number = /^[+-]?(?:[0-9]+(?:\.[0-9]*)?|\.[0-9]+)(?:[eE][+-]?[0-9]+)?$/;
export function calculate(a: string, op: string, b: string): number {
  if (a.length > 100 || b.length > 100 || !number.test(a) || !number.test(b))
    throw new Error("数値を入力してください");
  const left = Number(a), right = Number(b);
  if (!Number.isFinite(left) || !Number.isFinite(right)) throw new Error("有限の数値を入力してください");
  let result: number;
  switch (op) {
    case "add": result = left + right; break;
    case "sub": result = left - right; break;
    case "mul": result = left * right; break;
    case "div":
      if (right === 0) throw new Error("ゼロで除算できません");
      result = left / right; break;
    default: throw new Error("不明な演算子です");
  }
  if (!Number.isFinite(result)) throw new Error("結果が数値の範囲を超えています");
  return result;
}
