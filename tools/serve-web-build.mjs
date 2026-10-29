// Serve an existing build without running a second build/watch process.
import fs from 'node:fs';
import path from 'node:path';
import http from 'node:http';
const [directory,portArgument='8084']=process.argv.slice(2);
if(!directory)throw Error('Usage: node tools/serve-web-build.mjs <built-public-directory> [port]');
const root=fs.realpathSync(directory),port=Number(portArgument);
if(!Number.isInteger(port)||port<1024||port>65535)throw Error('Invalid local port');
if(!fs.statSync(path.join(root,'index.html')).isFile())throw Error('No built index.html');
const types={'.html':'text/html; charset=utf-8','.js':'text/javascript; charset=utf-8',
  '.mjs':'text/javascript; charset=utf-8','.wasm':'application/wasm',
  '.css':'text/css; charset=utf-8','.json':'application/json','.svg':'image/svg+xml',
  '.png':'image/png','.woff2':'font/woff2','.ico':'image/x-icon'};
const server=http.createServer((request,response)=>{
  response.setHeader('Cross-Origin-Opener-Policy','same-origin');
  response.setHeader('Cross-Origin-Embedder-Policy','require-corp');
  response.setHeader('Cache-Control','no-store');
  if(!['GET','HEAD'].includes(request.method)){response.writeHead(405);response.end();return;}
  try{
    const relative=decodeURIComponent(new URL(request.url,'http://localhost').pathname).replace(/^\/+/, '')||'index.html';
    const candidate=path.resolve(root,relative);
    if(!candidate.startsWith(root+path.sep)){response.writeHead(403);response.end();return;}
    const file=fs.realpathSync(candidate);
    if(!file.startsWith(root+path.sep)||!fs.statSync(file).isFile()){response.writeHead(404);response.end();return;}
    response.setHeader('Content-Type',types[path.extname(file)]??'application/octet-stream');
    response.setHeader('Content-Length',fs.statSync(file).size);
    if(request.method==='HEAD'){response.end();return;}
    const stream=fs.createReadStream(file);stream.on('error',()=>response.destroy());stream.pipe(response);
  }catch{response.writeHead(404);response.end();}
});
server.listen(port,'127.0.0.1',()=>console.log(`Serving existing build at http://127.0.0.1:${port}/ with isolation headers`));
process.on('SIGINT',()=>server.close());
