import Mustache from "mustache";
import template from "../shared/calculator.html";
import { calculate } from "./calculator.js";
import { Fields, OutgoingResponse, ResponseOutparam, OutgoingBody } from "wasi:http/types@0.2.12";

// Standard WASI HTTP resources; the arithmetic runs inside the embedded JS engine.
export const incomingHandler = {
  handle(request: any, out: any): void {
    const path = (request.pathWithQuery() || "").split("?")[0];
    if (path !== "/typescript" && path !== "/typescript/") {
      respond(out, 404, "Not found", "text/plain"); return;
    }
    const context = {
      language:"TypeScript", template:"Mustache.js 4.2.0", runtime:"StarlingMonkey / ComponentizeJS 0.23.0",
      version:"demo-v1", path:"/typescript", a:"", b:"", operation:"", result:"", error:""
    };
    let status = 200;
    if (request.method().tag === "post") {
      const body=request.consume(), stream=body.stream();
      const chunks: Uint8Array[]=[];
      let size=0;
      try {
        while(true) {
          const chunk=stream.blockingRead(4096n) as Uint8Array;
          chunks.push(chunk); size+=chunk.length;
          if(size>4096){respond(out,413,"Body too large","text/plain");return;}
        }
      } catch(e: any) {
        if(e.payload?.tag!=="closed" && e.tag!=="closed") throw e;
      } finally {
        stream[Symbol.dispose]();
        body[Symbol.dispose]();
      }
      try {
        const bytes=new Uint8Array(size);
        let offset=0;
        for(const chunk of chunks){bytes.set(chunk,offset);offset+=chunk.length;}
        const fields=new URLSearchParams(new TextDecoder("utf-8",{fatal:true}).decode(bytes));
        for(const key of ["a","b","op"])if(fields.getAll(key).length!==1)throw new Error("入力が不足または重複しています");
        context.a=fields.get("a")!;context.b=fields.get("b")!;context.operation=fields.get("op")!;
        context.result=Number(calculate(context.a,context.operation,context.b).toPrecision(12)).toString();
      } catch(e){context.error=e instanceof Error?e.message:String(e);status=400;}
    } else if(request.method().tag!=="get"){
      respond(out,405,"Method not allowed","text/plain");return;
    }
    respond(out,status,Mustache.render(template,context),"text/html; charset=utf-8");
  }
};
function respond(out:any,status:number,text:string,contentType:string){
  const headers=new Fields();
  headers.set("content-type",[new TextEncoder().encode(contentType)]);
  const response=new OutgoingResponse(headers);
  response.setStatusCode(status);
  const body=response.body();
  ResponseOutparam.set(out,{tag:"ok",val:response});
  const stream=body.write(), bytes=new TextEncoder().encode(text);
  for(let i=0;i<bytes.length;i+=4096)stream.blockingWriteAndFlush(bytes.subarray(i,i+4096));
  stream[Symbol.dispose]();
  OutgoingBody.finish(body,undefined);
}
