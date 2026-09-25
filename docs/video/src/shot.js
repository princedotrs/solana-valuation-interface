const {chromium}=require('playwright');
(async()=>{const b=await chromium.launch();const p=await b.newPage({viewport:{width:1920,height:1080},deviceScaleFactor:1});
for (const [u,n] of [['http://127.0.0.1:8099/site/index.html','site'],['http://127.0.0.1:8099/docs/product/index.html','product']]){
 await p.goto(u,{waitUntil:'load'}).catch(e=>console.log(e.message)); await p.waitForTimeout(2500);
 await p.screenshot({path:`build/shots/${n}_full.png`,fullPage:true}); await p.screenshot({path:`build/shots/${n}_top.png`});
 console.log(n, await p.evaluate(()=>document.body.scrollHeight));}
await b.close();})();
