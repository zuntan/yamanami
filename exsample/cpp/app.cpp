#include "calculator.h"
#include "calculator.hpp"
#include "template.hpp"
#include "vendor/mustache.hpp"
#include <map>
#include <sstream>
#include <iomanip>
#include <algorithm>
static calculator_string_t str(const std::string& s){return {reinterpret_cast<uint8_t*>(const_cast<char*>(s.data())),s.size()};}
static wasi_http_types_borrow_fields_t borrow(wasi_http_types_own_fields_t h){return {h.__handle};}
static wasi_http_types_borrow_incoming_request_t borrow(wasi_http_types_own_incoming_request_t h){return {h.__handle};}
static wasi_http_types_borrow_outgoing_response_t borrow(wasi_http_types_own_outgoing_response_t h){return {h.__handle};}
static wasi_http_types_borrow_incoming_body_t borrow(wasi_http_types_own_incoming_body_t h){return {h.__handle};}
static wasi_http_types_borrow_outgoing_body_t borrow(wasi_http_types_own_outgoing_body_t h){return {h.__handle};}

static void respond(wasi_http_types_own_response_outparam_t out,uint16_t status,const std::string& text,const std::string& contentType){
  auto fields=wasi_http_types_constructor_fields();
  std::string key="content-type";auto k=str(key);auto v=str(contentType);
  wasi_http_types_field_value_t bytes{v.ptr,v.len};
  wasi_http_types_list_field_value_t values{&bytes,1};
  wasi_http_types_header_error_t header_error;
  if(!wasi_http_types_method_fields_set(borrow(fields),&k,&values,&header_error))__builtin_trap();
  auto response=wasi_http_types_constructor_outgoing_response(fields);
  if(!wasi_http_types_method_outgoing_response_set_status_code(borrow(response),status))__builtin_trap();
  wasi_http_types_own_outgoing_body_t body;
  if(!wasi_http_types_method_outgoing_response_body(borrow(response),&body))__builtin_trap();
  wasi_http_types_result_own_outgoing_response_error_code_t result{};
  result.is_err=false;result.val.ok=response;
  wasi_http_types_static_response_outparam_set(out,&result);
  wasi_http_types_own_output_stream_t stream;
  if(!wasi_http_types_method_outgoing_body_write(borrow(body),&stream))__builtin_trap();
  for(size_t offset=0;offset<text.size();offset+=4096){
    calculator_list_u8_t data{reinterpret_cast<uint8_t*>(const_cast<char*>(text.data()+offset)),std::min(size_t(4096),text.size()-offset)};
    wasi_io_streams_stream_error_t error;
    if(!wasi_io_streams_method_output_stream_blocking_write_and_flush({stream.__handle},&data,&error))__builtin_trap();
  }
  wasi_io_streams_output_stream_drop_own(stream);
  wasi_http_types_error_code_t error;
  if(!wasi_http_types_static_outgoing_body_finish(body,nullptr,&error))__builtin_trap();
}
static int hex(char c){if(c>='0'&&c<='9')return c-'0';if(c>='a'&&c<='f')return c-'a'+10;if(c>='A'&&c<='F')return c-'A'+10;return -1;}
static bool decode(const std::string& s,std::string& out){
  for(size_t i=0;i<s.size();++i){
    if(s[i]=='+')out+=' ';
    else if(s[i]=='%'){
      if(i+2>=s.size()||hex(s[i+1])<0||hex(s[i+2])<0)return false;
      out+=char(hex(s[i+1])*16+hex(s[i+2]));i+=2;
    }else out+=s[i];
  }
  return true;
}
static bool parse_form(const std::string& text,std::map<std::string,std::string>& fields){
  size_t pos=0;
  while(pos<text.size()){
    size_t end=text.find('&',pos);if(end==std::string::npos)end=text.size();
    auto pair=text.substr(pos,end-pos);auto eq=pair.find('=');
    if(eq==std::string::npos)return false;
    std::string key,value;
    if(!decode(pair.substr(0,eq),key)||!decode(pair.substr(eq+1),value)||fields.count(key))return false;
    fields[key]=value;pos=end+1;
  }return true;
}
extern "C" void exports_wasi_http_incoming_handler_handle(
  exports_wasi_http_incoming_handler_own_incoming_request_t request,
  exports_wasi_http_incoming_handler_own_response_outparam_t out){
  calculator_string_t uri{};
  wasi_http_types_method_incoming_request_path_with_query(borrow(request),&uri);
  std::string path(reinterpret_cast<char*>(uri.ptr),uri.len);
  calculator_string_free(&uri);path=path.substr(0,path.find('?'));
  if(path!="/cpp"&&path!="/cpp/"){
    wasi_http_types_incoming_request_drop_own(request);respond(out,404,"Not found","text/plain");return;
  }
  std::map<std::string,std::string> context{
    {"language","C++"},{"template","kainjow/Mustache 4.1"},{"runtime","WASI SDK 34 / libc++"},
    {"path","/cpp"},{"version","demo-v1"},{"a",""},{"b",""},{"operation",""},{"result",""},{"error",""}};
  uint16_t status=200;wasi_http_types_method_t method;
  wasi_http_types_method_incoming_request_method(borrow(request),&method);
  if(method.tag==WASI_HTTP_TYPES_METHOD_POST){
    wasi_http_types_own_incoming_body_t body;wasi_http_types_own_input_stream_t stream;
    if(!wasi_http_types_method_incoming_request_consume(borrow(request),&body)||
       !wasi_http_types_method_incoming_body_stream(borrow(body),&stream))__builtin_trap();
    std::string text;
    while(true){
      calculator_list_u8_t chunk{};wasi_io_streams_stream_error_t error{};
      if(!wasi_io_streams_method_input_stream_blocking_read({stream.__handle},4096,&chunk,&error)){
        if(error.tag!=WASI_IO_STREAMS_STREAM_ERROR_CLOSED)__builtin_trap();break;
      }
      text.append(reinterpret_cast<char*>(chunk.ptr),chunk.len);calculator_list_u8_free(&chunk);
      if(text.size()>4096)break;
    }
    wasi_io_streams_input_stream_drop_own(stream);wasi_http_types_incoming_body_drop_own(body);
    std::map<std::string,std::string> fields;
    if(text.size()>4096||!parse_form(text,fields)){
      context["error"]="入力形式が正しくありません";status=400;
    }else{
      context["a"]=fields["a"];context["b"]=fields["b"];context["operation"]=fields["op"];
      auto result=calculate(fields["a"],fields["op"],fields["b"]);
      if(result.ok){std::ostringstream buffer;buffer<<std::setprecision(12)<<result.value;context["result"]=buffer.str();}
      else{context["error"]=result.error;status=400;}
    }
  }else if(method.tag!=WASI_HTTP_TYPES_METHOD_GET){
    wasi_http_types_method_free(&method);wasi_http_types_incoming_request_drop_own(request);
    respond(out,405,"Method not allowed","text/plain");return;
  }
  wasi_http_types_method_free(&method);wasi_http_types_incoming_request_drop_own(request);
  kainjow::mustache::mustache tmpl(CALCULATOR_TEMPLATE);
  kainjow::mustache::data data;
  for(const auto&[key,value]:context)data.set(key,value);
  respond(out,status,tmpl.render(data),"text/html; charset=utf-8");
}
