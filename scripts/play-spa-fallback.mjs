#!/usr/bin/env node
// GitHub Pages has no rewrite rules. A missing /play/<route> returns the
// site 404, which drops the workbench. This stamps a redirect onto 404.html:
// play routes stash the URL and load /play/, and the play bundle restores it.

import { existsSync, readFileSync, writeFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'

export const PLAY_REDIRECT_KEY = 'aqa-play-redirect'

export const PLAY_FALLBACK_SCRIPT = `<script>(function(){try{var path=location.pathname;if(/\\/play\\/?$/.test(path))return;var file=path.split('/').pop()||'';if(file.indexOf('.')!==-1)return;var at=path.indexOf('/play/');if(at===-1)return;var base=path.slice(0,at+'/play'.length);sessionStorage.setItem('${PLAY_REDIRECT_KEY}',path+location.search+location.hash);location.replace(base+'/');}catch(e){}})();</script>`

/** Prepend the play redirect to a site 404.html, or write a minimal one. */
export function writePlayFallback(distDir) {
  const file = join(distDir, '404.html')
  if (existsSync(file)) {
    const html = readFileSync(file, 'utf8')
    if (html.includes(PLAY_REDIRECT_KEY)) return file
    const next = html.includes('<head>')
      ? html.replace('<head>', `<head>\n    ${PLAY_FALLBACK_SCRIPT}`)
      : `${PLAY_FALLBACK_SCRIPT}\n${html}`
    writeFileSync(file, next)
    return file
  }
  writeFileSync(
    file,
    `<!doctype html><html><head>${PLAY_FALLBACK_SCRIPT}</head><body>Not found</body></html>\n`
  )
  return file
}

const isMain = process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href
if (isMain) {
  const dist = process.argv[2] || join(dirname(fileURLToPath(import.meta.url)), '..', 'site', 'dist')
  const file = writePlayFallback(dist)
  console.log(`play spa fallback → ${file}`)
}
