import { mkdir,readdir,readFile,rm,writeFile } from "node:fs/promises";
import { join,resolve } from "node:path";
import { fileURLToPath } from "node:url";
const root=resolve(fileURLToPath(new URL("..",import.meta.url)));const source=join(root,"src");const dist=join(root,"dist");await rm(dist,{recursive:true,force:true});await mkdir(dist,{recursive:true});for(const name of await readdir(source)){if(!name.endsWith(".ts"))continue;let content=await readFile(join(source,name),"utf8");content=content.replaceAll(/from\s+(["'][^"']+)\.ts(["'])/g,"from $1.js$2");await writeFile(join(dist,name.replace(/\.ts$/,".js")),content);}console.log("✅ Control Center build listo");
