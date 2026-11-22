import {createServer} from 'vite';
import {pathToFileURL} from 'node:url';
import {resolve} from 'node:path';
const server=await createServer({server:{host:'127.0.0.1',port:5183,strictPort:true,watch:{ignored:['**/target/**','**/all-artifacts/**']}}});
await server.listen();process.env.GRIT_TEST_URL='http://127.0.0.1:5183/';
try {await import(pathToFileURL(resolve(process.argv[2])).href);} finally {await server.close();}
