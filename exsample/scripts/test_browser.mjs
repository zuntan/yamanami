// Browser verification uses a fresh, isolated headless Chromium profile.
import { chromium } from "../typescript/node_modules/playwright/index.mjs";
import assert from "node:assert/strict";
import { mkdir } from "node:fs/promises";

const origin=process.argv[2] || "http://127.0.0.1:8080";
const browser=await chromium.launch({headless:true});
try {
  const page=await browser.newPage({viewport:{width:1440,height:1600}});
  await page.goto(origin,{waitUntil:"networkidle",timeout:30000});
  assert.equal(await page.locator("iframe").count(),4);
  for(const language of ["Rust","C++","Python","TypeScript"]){
    const frame=page.frameLocator('iframe[title="'+language+' calculator"]');
    await frame.locator("main").waitFor();
    await frame.getByLabel("左の数値").fill("6");
    await frame.getByLabel("右の数値").fill("7");
    await frame.getByLabel("演算").selectOption("mul");
    await frame.getByRole("button",{name:"計算する"}).click();
    await frame.locator("#result").filter({hasText:/^42$/}).waitFor({timeout:15000});
    await frame.getByLabel("右の数値").fill("0");
    await frame.getByLabel("演算").selectOption("div");
    await frame.getByRole("button",{name:"計算する"}).click();
    await frame.getByRole("status").filter({hasText:"ゼロ"}).waitFor({timeout:15000});
    await frame.getByRole("link",{name:"クリア"}).click();
    await frame.locator('input[name="a"][value=""]').waitFor({timeout:15000});
    assert.equal(await frame.getByLabel("左の数値").inputValue(),"");
    assert.equal(await frame.locator("#result").textContent(),"");
    console.log(language+": iframe calculation, error, clear OK");
  }
  await mkdir(new URL("../build/",import.meta.url),{recursive:true});
  await page.screenshot({path:new URL("../build/demo-desktop.png",import.meta.url).pathname,fullPage:true});
  await page.setViewportSize({width:390,height:844});
  await page.screenshot({path:new URL("../build/demo-mobile.png",import.meta.url).pathname,fullPage:true});
  assert.equal(await page.locator("main").first().evaluate(el=>getComputedStyle(el).gridTemplateColumns.split(" ").length),1);
  console.log("Responsive index layout OK");
}finally{await browser.close();}
