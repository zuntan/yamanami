import {test} from "node:test";
import assert from "node:assert/strict";
import {calculate} from "../calculator.js";
test("four operations",()=>{
  for(const [a,op,b,expected] of [["2","add","3",5],["7","sub","9",-2],["-2.5","mul","4",-10],["9","div","4",2.25]] as const)
    assert.equal(calculate(a,op,b),expected);
});
test("invalid input and arithmetic errors",()=>{
  for(const [a,op,b] of [["1","div","0"],["1","div","-0"],["NaN","add","1"],["1e309","add","1"],["","add","2"],["1x","add","2"],["1","eval","2"],["1e308","mul","1e308"]])
    assert.throws(()=>calculate(a!,op!,b!));
});
