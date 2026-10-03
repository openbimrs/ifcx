import { Buffer } from 'node:buffer'

import type MarkdownIt from 'markdown-it'

function encoded(content: string): string {
  return Buffer.from(content, 'utf8').toString('base64')
}

export function diagramPlugin(md: MarkdownIt): void {
  const defaultFence = md.renderer.rules.fence
  if (!defaultFence) throw new Error('VitePress Markdown renderer has no fence rule')

  md.renderer.rules.fence = (tokens, index, options, env, self) => {
    if (tokens[index].info.trim().split(/\s+/, 1)[0] === 'mermaid') {
      const source = tokens[index].content
      const wide = /^\s*(?:flowchart|graph)\s+(?:LR|RL|BT)\b/m.test(source)
      return `<MermaidDiagram encoded="${encoded(source)}"${wide ? ' wide' : ''} />`
    }
    return defaultFence(tokens, index, options, env, self)
  }
}
