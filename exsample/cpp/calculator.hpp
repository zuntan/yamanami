#pragma once
#include <cmath>
#include <cstdlib>
#include <string>
struct Calculation { bool ok; double value; std::string error; };
inline bool valid_number(const std::string& s) {
  if(s.empty()||s.size()>100)return false;
  size_t i=0;if(s[i]=='+'||s[i]=='-')++i;
  size_t digits=0;while(i<s.size()&&s[i]>='0'&&s[i]<='9'){++i;++digits;}
  if(i<s.size()&&s[i]=='.'){++i;while(i<s.size()&&s[i]>='0'&&s[i]<='9'){++i;++digits;}}
  if(!digits)return false;
  if(i<s.size()&&(s[i]=='e'||s[i]=='E')){
    ++i;if(i<s.size()&&(s[i]=='+'||s[i]=='-'))++i;
    size_t start=i;while(i<s.size()&&s[i]>='0'&&s[i]<='9')++i;
    if(i==start)return false;
  }
  return i==s.size();
}
inline Calculation calculate(const std::string& a,const std::string& op,const std::string& b){
  if(!valid_number(a)||!valid_number(b))return {false,0,"数値を入力してください"};
  double left=std::strtod(a.c_str(),nullptr),right=std::strtod(b.c_str(),nullptr),result=0;
  if(!std::isfinite(left)||!std::isfinite(right))return {false,0,"有限の数値を入力してください"};
  if(op=="add")result=left+right;
  else if(op=="sub")result=left-right;
  else if(op=="mul")result=left*right;
  else if(op=="div"){if(right==0)return {false,0,"ゼロで除算できません"};result=left/right;}
  else return {false,0,"不明な演算子です"};
  if(!std::isfinite(result))return {false,0,"結果が数値の範囲を超えています"};
  return {true,result,{}};
}
