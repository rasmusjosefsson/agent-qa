import { PanelResizeHandle } from 'react-resizable-panels'

// Flush 1px vertical divider that doubles as a horizontal drag handle. The
// visible line stays 1px (edge-to-edge look); a wider invisible ::after gives
// a comfortable grab target. Highlights on hover/drag.
export function ResizeHandle() {
  return (
    <PanelResizeHandle className="relative w-px shrink-0 cursor-col-resize bg-border transition-colors after:absolute after:inset-y-0 after:left-1/2 after:w-2 after:-translate-x-1/2 after:content-[''] hover:bg-primary data-[resize-handle-state=drag]:bg-primary data-[resize-handle-state=hover]:bg-primary" />
  )
}
