import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const entries=path=>readFileSync(path,'utf8').split('[[package]]').flatMap(text=>{
  const name=text.match(/^name = "([^"]+)"/m)?.[1],version=text.match(/^version = "([^"]+)"/m)?.[1],checksum=text.match(/^checksum = "([^"]+)"/m)?.[1];
  const dependencies=(text.match(/^dependencies = \[([\s\S]*?)\]/m)?.[1] || '').match(/"([^"]+)"/g)?.map(d=>d.slice(1,-1)) || [];
  return name && version ? [{name,version,checksum,dependencies}] : [];
});
const application=entries('Cargo.lock'),fuzz=entries('fuzz/Cargo.lock');
function reachable(packages){
  const found=new Set(),queue=packages.filter(p=>p.name==='telex-core');
  while(queue.length){
    const current=queue.pop(); if(found.has(current)) continue; found.add(current);
    for(const dependency of current.dependencies){
      const [name,version]=dependency.split(' ');
      queue.push(...packages.filter(p=>p.name===name && (!version || p.version===version)));
    }
  }
  return [...found];
}
const appCore=reachable(application),fuzzCore=reachable(fuzz);
for(const name of ['telex-core','ammonia','mail-parser','outlook-pst','cfb','html5ever','cssparser']){
  const versions=packages=>packages.filter(p=>p.name===name).map(({name,version,checksum})=>({name,version,checksum})).sort((a,b)=>a.version.localeCompare(b.version));
  assert.ok(versions(appCore).length>0,`${name}: expected application parser dependency`);
  assert.deepEqual(versions(fuzzCore),versions(appCore),`${name}: fuzz and application dependencies differ`);
}
console.log('Fuzz targets use the application’s locked MIME, HTML and PST parser versions.');
