// web/src/features/chat/chatReducer.test.ts
import { describe, it, expect } from 'vitest'
import { reducer, initialState, toolSummary, type Action, type ChatUIState } from './chatReducer'
import type { AgentEvent, ChatState } from '@/lib/types'

function run(events: Action[], start: ChatUIState = initialState): ChatUIState {
  return events.reduce((s, a) => reducer(s, a), start)
}

const ev = (event: AgentEvent): Action => ({ type: 'agent_event', event })

describe('chatReducer — live streaming fold', () => {
  it('folds text_start + text_delta* into a single assistant bubble', () => {
    const s = run([
      ev({ type: 'agent_start' }),
      ev({ type: 'message_update', assistantMessageEvent: { type: 'text_start' } }),
      ev({ type: 'message_update', assistantMessageEvent: { type: 'text_delta', delta: 'Hel' } }),
      ev({ type: 'message_update', assistantMessageEvent: { type: 'text_delta', delta: 'lo' } }),
      ev({ type: 'message_end' }),
      ev({ type: 'agent_end' }),
    ])
    expect(s.items).toHaveLength(1)
    expect(s.items[0]).toMatchObject({ kind: 'assistant', text: 'Hello' })
    expect(s.streaming).toBe(false)
    expect(s.curAssistant).toBeNull()
  })

  it('keeps thinking and text as separate ordered bubbles', () => {
    const s = run([
      ev({ type: 'message_update', assistantMessageEvent: { type: 'thinking_delta', delta: 'hmm ' } }),
      ev({ type: 'message_update', assistantMessageEvent: { type: 'thinking_delta', delta: 'ok' } }),
      ev({ type: 'message_update', assistantMessageEvent: { type: 'text_delta', delta: 'answer' } }),
    ])
    expect(s.items.map((i) => i.kind)).toEqual(['thinking', 'assistant'])
    expect(s.items[0]).toMatchObject({ kind: 'thinking', text: 'hmm ok' })
    expect(s.items[1]).toMatchObject({ kind: 'assistant', text: 'answer' })
  })

  it('opens a fresh assistant bubble after a tool card (interleaving)', () => {
    const s = run([
      ev({ type: 'message_update', assistantMessageEvent: { type: 'text_delta', delta: 'before' } }),
      ev({ type: 'tool_execution_start', toolName: 'run', args: { a: 1 }, toolCallId: 't1' }),
      ev({ type: 'tool_execution_update', toolCallId: 't1', partialResult: { content: [{ type: 'text', text: 'partial' }] } }),
      ev({ type: 'tool_execution_end', toolCallId: 't1', isError: false, result: { content: [{ type: 'text', text: 'final out' }] } }),
      ev({ type: 'message_update', assistantMessageEvent: { type: 'text_delta', delta: 'after' } }),
    ])
    expect(s.items.map((i) => i.kind)).toEqual(['assistant', 'tool', 'assistant'])
    expect(s.items[0]).toMatchObject({ text: 'before' })
    expect(s.items[1]).toMatchObject({ kind: 'tool', name: 'run', status: 'ok', out: 'final out' })
    expect(s.items[2]).toMatchObject({ text: 'after' })
  })

  it('tool_execution_update replaces (not appends) partial output', () => {
    const s = run([
      ev({ type: 'tool_execution_start', toolName: 'run', toolCallId: 't1' }),
      ev({ type: 'tool_execution_update', toolCallId: 't1', partialResult: 'one' }),
      ev({ type: 'tool_execution_update', toolCallId: 't1', partialResult: 'one-two' }),
    ])
    expect(s.items[0]).toMatchObject({ kind: 'tool', out: 'one-two', status: 'running' })
  })

  it('marks tool cards as error on isError', () => {
    const s = run([
      ev({ type: 'tool_execution_start', toolName: 'run', toolCallId: 't1' }),
      ev({ type: 'tool_execution_end', toolCallId: 't1', isError: true, result: 'boom' }),
    ])
    expect(s.items[0]).toMatchObject({ kind: 'tool', status: 'err', out: 'boom' })
  })

  it('creates a tool card on tool_execution_end when start was missed', () => {
    const s = run([
      ev({ type: 'tool_execution_end', toolCallId: 't9', toolName: 'late', result: 'done' }),
    ])
    expect(s.items).toHaveLength(1)
    expect(s.items[0]).toMatchObject({ kind: 'tool', name: 'late', status: 'ok', out: 'done' })
  })

  it('pushes an error item and clears streaming on agent error', () => {
    const s = run([
      ev({ type: 'agent_start' }),
      ev({ type: 'error', message: 'kaboom' }),
    ])
    expect(s.items[0]).toMatchObject({ kind: 'error', text: 'kaboom' })
    expect(s.streaming).toBe(false)
  })
})

describe('chatReducer — rehydration (init_state)', () => {
  const saved: ChatState = {
    available: true,
    streaming: false,
    model: { provider: 'anthropic', id: 'sonnet', label: 'Sonnet' },
    models: [{ provider: 'anthropic', id: 'sonnet' }],
    thinkingLevel: 'off',
    thinkingLevels: ['off', 'low', 'high'],
    messages: [
      { role: 'user', content: 'hi' },
      {
        role: 'assistant',
        content: [
          { type: 'thinking', thinking: 'pondering' },
          { type: 'text', text: 'hello there' },
          { type: 'toolCall', id: 'tc1', name: 'list', arguments: { x: 1 } },
        ],
      },
      { role: 'toolResult', toolCallId: 'tc1', toolName: 'list', isError: false, content: [{ type: 'text', text: 'a,b,c' }] },
    ],
  }

  it('rebuilds ordered items and finalizes tool results', () => {
    const s = reducer(initialState, { type: 'init_state', payload: saved })
    expect(s.items.map((i) => i.kind)).toEqual(['user', 'thinking', 'assistant', 'tool'])
    expect(s.items[0]).toMatchObject({ kind: 'user', text: 'hi' })
    expect(s.items[2]).toMatchObject({ kind: 'assistant', text: 'hello there' })
    expect(s.items[3]).toMatchObject({ kind: 'tool', name: 'list', status: 'ok', out: 'a,b,c' })
    expect(s.available).toBe(true)
    expect(s.model?.id).toBe('sonnet')
    expect(s.curAssistant).toBeNull()
  })

  it('handles user content as text-part arrays', () => {
    const s = reducer(initialState, {
      type: 'init_state',
      payload: { available: true, messages: [{ role: 'user', content: [{ type: 'text', text: 'arr-form' }] }] },
    })
    expect(s.items[0]).toMatchObject({ kind: 'user', text: 'arr-form' })
  })
})

describe('chatReducer — meta + lifecycle', () => {
  it('patch_meta updates model/thinking without touching items', () => {
    const base = reducer(initialState, { type: 'add_user', text: 'keep me' })
    const s = reducer(base, {
      type: 'patch_meta',
      payload: { model: { id: 'opus' }, thinkingLevel: 'high', thinkingLevels: ['off', 'high'] },
    })
    expect(s.items).toHaveLength(1)
    expect(s.model?.id).toBe('opus')
    expect(s.thinkingLevel).toBe('high')
  })

  it('new_chat clears the thread', () => {
    const base = run([
      ev({ type: 'message_update', assistantMessageEvent: { type: 'text_delta', delta: 'x' } }),
    ])
    const s = reducer(base, { type: 'new_chat' })
    expect(s.items).toHaveLength(0)
    expect(s.toolIndex).toEqual({})
    expect(s.streaming).toBe(false)
  })

  it('session_idle sets a note and stops streaming', () => {
    const s = reducer({ ...initialState, streaming: true }, { type: 'session_event', event: { type: 'session_idle' } })
    expect(s.streaming).toBe(false)
    expect(s.sessionNote).toMatch(/idled out/i)
  })
})

describe('toolSummary — compact tool titles', () => {
  it('prefers an explicit description arg', () => {
    expect(toolSummary('bash', { command: 'cat skill/core/SKILL.md', description: 'Read skill core' })).toBe(
      'Read skill core'
    )
  })

  it('summarizes a bash command to its first line when no description', () => {
    expect(toolSummary('bash', { command: 'npm test\nsecond line' })).toBe('$ npm test')
  })

  it('uses a leading comment as the title for multi-line scripts (skips the blank first line)', () => {
    const command = '\n# Navigate to the accounts page\nagent-browser open "https://x"\nsleep 2'
    expect(toolSummary('bash', { command })).toBe('Navigate to the accounts page')
  })

  it('falls back to the first real command when there is no leading comment', () => {
    expect(toolSummary('bash', { command: '\n\nls -la\n# trailing' })).toBe('$ ls -la')
  })

  it('uses the file basename for file tools (accepts file_path and path)', () => {
    expect(toolSummary('edit', { file_path: '/x/ToolCard.tsx' })).toBe('Edit ToolCard.tsx')
    // pi agent's Read tool names the arg `path`, not `file_path`.
    expect(toolSummary('Read', { path: '/x/ToolCard.tsx' })).toBe('Read ToolCard.tsx')
  })

  it('includes the parent dir for ambiguous filenames like SKILL.md', () => {
    expect(toolSummary('Read', { path: '/Users/x/.pi/agent/skills/agent-qa/SKILL.md' })).toBe(
      'Read agent-qa/SKILL.md'
    )
    expect(toolSummary('Read', { file_path: '/repo/src/index.ts' })).toBe('Read src/index.ts')
  })

  it('handles grep, skill, agent, and web tools', () => {
    expect(toolSummary('grep', { pattern: 'TODO' })).toBe('Grep TODO')
    expect(toolSummary('Skill', { skill: 'agent-qa' })).toBe('Skill agent-qa')
    expect(toolSummary('Task', { subagent_type: 'Explore' })).toBe('Explore')
    expect(toolSummary('webfetch', { url: 'https://example.com/docs/x' })).toBe('Fetch example.com')
  })

  it('clips long descriptions and falls back to the raw name', () => {
    expect(toolSummary('mystery', {})).toBe('mystery')
    expect(toolSummary('bash', { description: 'x'.repeat(200) }).endsWith('…')).toBe(true)
  })
})
