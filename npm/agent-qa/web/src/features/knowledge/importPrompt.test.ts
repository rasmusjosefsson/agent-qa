// web/src/features/knowledge/importPrompt.test.ts
import { describe, it, expect } from 'vitest'
import {
  buildJiraImportPrompt,
  buildXrayImportPrompt,
  caseIdFromKey,
  setIdFromKey,
} from './importPrompt'

describe('caseIdFromKey', () => {
  it('lowercases and safe-slugs an issue key', () => {
    expect(caseIdFromKey('PROJ-123')).toBe('proj-123')
    expect(caseIdFromKey('  ABC_9 ')).toBe('abc-9')
  })
})

describe('buildJiraImportPrompt', () => {
  const p = buildJiraImportPrompt('PROJ-123', 'http://127.0.0.1:7878')
  it('names the issue and the derived case id', () => {
    expect(p).toContain('Jira issue PROJ-123')
    expect(p).toContain('http://127.0.0.1:7878/api/cases/proj-123')
  })
  it('maps issue fields to case fields and tags the source', () => {
    expect(p).toContain('title')
    expect(p).toContain('expected')
    expect(p).toContain('"source":"jira"')
    expect(p).toContain('"sourceRef":"PROJ-123"')
  })
})

describe('setIdFromKey', () => {
  it('prefixes a safe-slugged container key', () => {
    expect(setIdFromKey('PROJ-123')).toBe('set-proj-123')
  })
})

describe('buildXrayImportPrompt', () => {
  const p = buildXrayImportPrompt('PROJ-123', 'plan', 'http://127.0.0.1:7878')
  it('names the container and scopes to its member tests', () => {
    expect(p).toContain('Xray test plan PROJ-123')
    expect(p).toContain('member tests')
  })
  it('writes cases + a set with provider-agnostic external refs', () => {
    expect(p).toContain('/api/cases/<case-id>')
    expect(p).toContain('http://127.0.0.1:7878/api/sets/set-proj-123')
    expect(p).toContain('"source":"xray"')
    expect(p).toContain('"provider":"xray"')
  })
  it('adapts wording for a story', () => {
    const s = buildXrayImportPrompt('PROJ-9', 'story', 'http://x')
    expect(s).toContain('Xray story PROJ-9')
    expect(s).toContain('tests covering it')
  })
})
