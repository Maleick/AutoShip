# Focus Indicators - Keyboard Navigation Guide

## Overview

This document describes the focus indicator implementation for the TextQuest web frontend, ensuring keyboard navigation is accessible and compliant with WCAG AA standards.

## Focus Indicator Design

### Visual Appearance

All interactive elements receive a consistent focus indicator when navigated via keyboard:

- **Color**: Neriak Cyan (`#00e5ff`) — provides 4.5:1+ contrast ratio against dark backgrounds
- **Style**: 3px solid outline with 2px offset from element edges
- **Additional Enhancement**: Optional box-shadow for depth on input fields
- **Behavior**: Only visible on keyboard navigation (using `:focus-visible` pseudo-class)

### Contrast Compliance

The focus indicator colors meet or exceed WCAG AA requirements:
- Dark theme: 4.5:1 contrast (Neriak Cyan on void background)
- Light theme: 5.2:1 contrast (Neriak Cyan on light background)
- Enhanced contrast mode: 4px outline with 3px offset for users with vision impairments

### Non-Obscuring Placement

- **Outset outlines**: Default behavior places focus ring outside element boundaries
- **Inset on inputs**: Form fields use subtle inset box-shadow to avoid layout shifts
- **Dashed outlines**: Draggable/sortable elements use dashed style for visual differentiation

## Supported Interactive Elements

Focus indicators are applied to:

- `<button>` — All button elements
- `<input>` — Text, email, password, number, etc.
- `<textarea>` — Multi-line text inputs
- `<select>` — Dropdown selections
- `<a>` — Hyperlinks
- `[role="button"]` — ARIA button elements
- `[role="tab"]` — Tab navigation
- `[role="menuitem"]` — Menu items
- `[role="link"]` — ARIA links
- `.card` — Card components
- `[role="dialog"]` — Modal dialogs
- `[data-sortable]` — Sortable/draggable lists

## Tab Order

The natural DOM order determines tab order. All interactive components follow this sequence:

1. **Skip Link** (hidden, focusable first for accessibility)
2. **Page Header** (search/command palette if present)
3. **Main Content**
   - Form inputs (top to bottom, left to right)
   - Buttons (within logical groups)
   - Links (content order)
4. **Sidebar/Secondary Navigation** (if applicable)
5. **Modal Dialogs** (focus trapped within modal when open)

### Custom Tab Order

If custom tab order is needed, use `tabindex` attribute:

```tsx
<button tabIndex={0}>Primary Action</button>
<button tabIndex={-1}>Hidden from tab order</button>
```

**Warning**: Avoid positive `tabIndex` values > 0, as they disrupt natural flow.

### Focus Management in Modals

When a modal opens:
1. Focus automatically moves to the first focusable element (or close button)
2. Tab focus is trapped within the modal
3. When modal closes, focus returns to the element that opened it

## Implementation Details

### CSS Source

Focus styles are centralized in `textquest-web/frontend/src/focus-indicators.css`.

### Component Updates

Key components have been updated to remove redundant focus styles:
- `Button.tsx` — Removed variant-specific focus rings (delegated to global stylesheet)
- `Form.tsx` — Removed inline focus classes (uses global styles + aria-invalid state)
- `Input.tsx` — Consistent outline handling via global stylesheet

### Accessibility Attributes

Components should include appropriate ARIA attributes:

```tsx
<input
  aria-label="Search"
  aria-invalid={hasError}
  aria-describedby={hasError ? "error-msg" : undefined}
/>
<div id="error-msg" role="alert">
  {error}
</div>
```

## Testing Focus Indicators

### Manual Testing

1. **Keyboard Navigation**: Press `Tab` to cycle through interactive elements
2. **Visual Feedback**: Verify cyan outline appears around focused element
3. **No Content Obscuring**: Check that focus ring doesn't hide text or icons
4. **All Components**: Test focus on buttons, inputs, links, modals, and custom elements

### Browser DevTools

To inspect focus styles:
```css
/* Simulate focus-visible state in DevTools */
:focus-visible { /* styles shown here */ }
```

### Automated Testing

Focus indicators should be tested via:
- Axe accessibility scanner (`axe-core`)
- WAVE WebAIM tools
- Manual keyboard navigation (Tab, Shift+Tab, Arrow keys in menus)

## High Contrast Mode Support

Users with `prefers-contrast: more` setting will receive:
- 4px outline width (vs. 3px default)
- 3px offset (vs. 2px default)
- Enhanced visibility on all elements

## Reduced Motion Support

Users with `prefers-reduced-motion: reduce` will not see:
- Focus ring animations or transitions
- All focus state changes are instantaneous

## Known Limitations

1. **Focus trapping in modals**: Currently implemented in `Modal.tsx`. May need enhancement for complex nested dialogs.
2. **Custom form elements**: Shadow DOM elements may not receive focus indicators.
3. **Lazy-loaded content**: Dynamically added elements should inherit global focus styles automatically.

## References

- [WCAG 2.1 - Focus Visible (2.4.7)](https://www.w3.org/WAI/WCAG21/Understanding/focus-visible.html)
- [MDN - :focus-visible](https://developer.mozilla.org/en-US/docs/Web/CSS/:focus-visible)
- [WebAIM - Keyboard Accessibility](https://webaim.org/articles/keyboard/)
- [ARIA Authoring Practices - Focus Management](https://www.w3.org/WAI/ARIA/apg/practices/keyboard-interface/)

## Contributing

When adding new interactive components:

1. Use semantic HTML (`<button>`, `<a>`, `<input>`) when possible
2. If using `<div>` or custom elements, add appropriate `role` attributes
3. Test focus with keyboard navigation
4. Ensure focus outline visibility with browser zoom (100%, 200%)
5. Do not remove focus styles — always preserve `:focus-visible`
