// Shared JSON GET. Same error string the Runs client has always thrown.

export async function fetchJson<T>(url: string, doFetch: typeof fetch = fetch): Promise<T> {
  const res = await doFetch(url, { headers: { accept: 'application/json' } })
  if (!res.ok) throw new Error(`${url} → ${res.status}`)
  return (await res.json()) as T
}

export async function fetchOptional(
  url: string,
  doFetch: typeof fetch = fetch,
): Promise<Response | null> {
  const res = await doFetch(url, { headers: { accept: 'application/json, text/plain' } })
  if (res.status === 404) return null
  if (!res.ok) throw new Error(`${url} → ${res.status}`)
  return res
}

export async function readArtifactText(url: string, pretty: boolean, doFetch: typeof fetch = fetch): Promise<string | null> {
  const res = await doFetch(url)
  if (!res.ok) return null
  let text = await res.text()
  if (pretty) {
    try {
      text = JSON.stringify(JSON.parse(text), null, 2)
    } catch {
      /* leave raw */
    }
  }
  return text
}
