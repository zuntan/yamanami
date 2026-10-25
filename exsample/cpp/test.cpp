#include "calculator.hpp"
#include <cassert>
int main(){
  assert(calculate("2","add","3").value==5);
  assert(calculate("7","sub","9").value==-2);
  assert(calculate("-2.5","mul","4").value==-10);
  assert(calculate("9","div","4").value==2.25);
  for(auto input:{"","NaN","Infinity","1x","1e309","1.2.3"})assert(!calculate(input,"add","1").ok);
  assert(!calculate("1","div","0").ok);
  assert(!calculate("1","div","-0").ok);
  assert(!calculate("1e308","mul","1e308").ok);
  assert(!calculate("1","eval","2").ok);
}
