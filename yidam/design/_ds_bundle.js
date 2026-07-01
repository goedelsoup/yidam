/* @ds-bundle: {"format":3,"namespace":"YidamDesignSystem_76df35","components":[{"name":"Avatar","sourcePath":"components/core/Avatar.jsx"},{"name":"Badge","sourcePath":"components/core/Badge.jsx"},{"name":"Button","sourcePath":"components/core/Button.jsx"},{"name":"Card","sourcePath":"components/core/Card.jsx"},{"name":"Tag","sourcePath":"components/core/Tag.jsx"},{"name":"Dialog","sourcePath":"components/feedback/Dialog.jsx"},{"name":"Toast","sourcePath":"components/feedback/Toast.jsx"},{"name":"Checkbox","sourcePath":"components/forms/Checkbox.jsx"},{"name":"Input","sourcePath":"components/forms/Input.jsx"},{"name":"Radio","sourcePath":"components/forms/Radio.jsx"},{"name":"Select","sourcePath":"components/forms/Select.jsx"},{"name":"Switch","sourcePath":"components/forms/Switch.jsx"},{"name":"Textarea","sourcePath":"components/forms/Textarea.jsx"},{"name":"BranchRef","sourcePath":"components/knowledge/BranchRef.jsx"},{"name":"ClaimMarker","sourcePath":"components/knowledge/ClaimMarker.jsx"},{"name":"NodeCard","sourcePath":"components/knowledge/NodeCard.jsx"},{"name":"PhaseTag","sourcePath":"components/knowledge/PhaseTag.jsx"},{"name":"Breadcrumb","sourcePath":"components/navigation/Breadcrumb.jsx"},{"name":"Tabs","sourcePath":"components/navigation/Tabs.jsx"},{"name":"Tooltip","sourcePath":"components/navigation/Tooltip.jsx"}],"sourceHashes":{"components/core/Avatar.jsx":"40fa38d82443","components/core/Badge.jsx":"62b15e65d857","components/core/Button.jsx":"328e78c3d7a2","components/core/Card.jsx":"ea39de485399","components/core/Tag.jsx":"561df1b2e467","components/feedback/Dialog.jsx":"0becf16f8399","components/feedback/Toast.jsx":"6b84e35f06f0","components/forms/Checkbox.jsx":"4a2e4ad0b309","components/forms/Input.jsx":"e61ac6cd8a66","components/forms/Radio.jsx":"92c8590bf7a7","components/forms/Select.jsx":"385891686922","components/forms/Switch.jsx":"db79f06bb0da","components/forms/Textarea.jsx":"9e56ca4ca39b","components/knowledge/BranchRef.jsx":"bd88ce7e72ef","components/knowledge/ClaimMarker.jsx":"55e881c9a950","components/knowledge/NodeCard.jsx":"5afe30b752dd","components/knowledge/PhaseTag.jsx":"53ef3aa93575","components/navigation/Breadcrumb.jsx":"b44fa94a14ab","components/navigation/Tabs.jsx":"35575d2710be","components/navigation/Tooltip.jsx":"11321a140d03"},"inlinedExternals":[],"unexposedExports":[]} */

(() => {

const __ds_ns = (window.YidamDesignSystem_76df35 = window.YidamDesignSystem_76df35 || {});

const __ds_scope = {};

(__ds_ns.__errors = __ds_ns.__errors || []);

// components/core/Avatar.jsx
try { (() => {
function initials(name) {
  if (!name) return '?';
  const parts = name.trim().split(/[\s\/]+/);
  if (parts.length >= 2) return (parts[0][0] + parts[1][0]).toUpperCase();
  return parts[0].slice(0, 2).toUpperCase();
}
const SIZES = {
  xs: {
    size: '24px',
    font: 'var(--text-2xs)'
  },
  sm: {
    size: '32px',
    font: 'var(--text-xs)'
  },
  md: {
    size: '40px',
    font: 'var(--text-sm)'
  },
  lg: {
    size: '52px',
    font: 'var(--text-base)'
  }
};
function Avatar({
  name,
  src,
  size = 'md',
  variant = 'human',
  style: styleProp
}) {
  const s = SIZES[size] || SIZES.md;
  const isAgent = variant === 'agent';
  const baseStyle = {
    width: s.size,
    height: s.size,
    borderRadius: isAgent ? 'var(--radius-md)' : '50%',
    display: 'inline-flex',
    alignItems: 'center',
    justifyContent: 'center',
    fontFamily: 'var(--font-ui)',
    fontSize: s.font,
    fontWeight: 500,
    flexShrink: 0,
    overflow: 'hidden',
    background: isAgent ? 'var(--rigpa-100)' : 'var(--ma-100)',
    color: isAgent ? 'var(--rigpa-800)' : 'var(--ma-800)',
    border: `1px solid ${isAgent ? 'var(--rigpa-200)' : 'var(--ma-200)'}`,
    userSelect: 'none',
    ...styleProp
  };
  if (src) {
    return /*#__PURE__*/React.createElement("span", {
      style: baseStyle
    }, /*#__PURE__*/React.createElement("img", {
      src: src,
      alt: name || '',
      style: {
        width: '100%',
        height: '100%',
        objectFit: 'cover',
        display: 'block'
      }
    }));
  }
  return /*#__PURE__*/React.createElement("span", {
    style: baseStyle
  }, initials(name));
}
Object.assign(__ds_scope, { Avatar });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/core/Avatar.jsx", error: String((e && e.message) || e) }); }

// components/core/Badge.jsx
try { (() => {
const VARIANTS = {
  default: {
    background: 'var(--ink-100)',
    color: 'var(--text-secondary)',
    border: '1px solid var(--ink-200)'
  },
  gold: {
    background: 'var(--gold-50)',
    color: 'var(--gold-800)',
    border: '1px solid var(--gold-200)'
  },
  rigpa: {
    background: 'var(--rigpa-50)',
    color: 'var(--rigpa-800)',
    border: '1px solid var(--rigpa-200)'
  },
  ma: {
    background: 'var(--ma-50)',
    color: 'var(--ma-800)',
    border: '1px solid var(--ma-200)'
  },
  verified: {
    background: 'var(--verified-bg)',
    color: 'var(--verified-fg)',
    border: '1px solid var(--verified-border)'
  },
  inference: {
    background: 'var(--inference-bg)',
    color: 'var(--inference-fg)',
    border: '1px solid var(--inference-border)'
  },
  open: {
    background: 'var(--open-bg)',
    color: 'var(--open-fg)',
    border: '1px solid var(--open-border)'
  },
  inverse: {
    background: 'var(--surface-inverse)',
    color: 'var(--text-inverse)',
    border: '1px solid transparent'
  }
};
function Badge({
  variant = 'default',
  size = 'md',
  dot = false,
  children
}) {
  const sizeStyle = size === 'sm' ? {
    padding: '1px 5px',
    fontSize: 'var(--text-2xs)'
  } : {
    padding: '2px 7px',
    fontSize: 'var(--text-xs)'
  };
  return /*#__PURE__*/React.createElement("span", {
    style: {
      display: 'inline-flex',
      alignItems: 'center',
      gap: '4px',
      borderRadius: 'var(--radius-sm)',
      fontFamily: 'var(--font-ui)',
      fontWeight: 500,
      whiteSpace: 'nowrap',
      lineHeight: 1.4,
      ...sizeStyle,
      ...(VARIANTS[variant] || VARIANTS.default)
    }
  }, dot && /*#__PURE__*/React.createElement("span", {
    style: {
      width: '5px',
      height: '5px',
      borderRadius: '50%',
      background: 'currentColor',
      flexShrink: 0
    }
  }), children);
}
Object.assign(__ds_scope, { Badge });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/core/Badge.jsx", error: String((e && e.message) || e) }); }

// components/core/Button.jsx
try { (() => {
function _extends() { return _extends = Object.assign ? Object.assign.bind() : function (n) { for (var e = 1; e < arguments.length; e++) { var t = arguments[e]; for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]); } return n; }, _extends.apply(null, arguments); }
const {
  useState
} = React;
const SIZES = {
  sm: {
    padding: '5px 12px',
    fontSize: 'var(--text-xs)',
    minHeight: '30px'
  },
  md: {
    padding: '7px 16px',
    fontSize: 'var(--text-sm)',
    minHeight: '36px'
  },
  lg: {
    padding: '10px 20px',
    fontSize: 'var(--text-base)',
    minHeight: '42px'
  }
};
function getVariantStyle(variant, hovered, pressed) {
  if (variant === 'primary') return {
    background: pressed ? 'var(--action-bg-active)' : hovered ? 'var(--action-bg-hover)' : 'var(--action-bg)',
    color: 'var(--action-fg)',
    borderColor: pressed ? 'var(--action-bg-active)' : hovered ? 'var(--action-bg-hover)' : 'var(--action-bg)'
  };
  if (variant === 'ghost') return {
    background: pressed ? 'var(--action-ghost-bg-active)' : hovered ? 'var(--action-ghost-bg-hover)' : 'transparent',
    color: 'var(--action-ghost-fg)',
    borderColor: 'var(--action-ghost-border)'
  };
  if (variant === 'subtle') return {
    background: pressed ? 'var(--ink-100)' : hovered ? 'var(--ink-50)' : 'transparent',
    color: 'var(--text-secondary)',
    borderColor: 'transparent'
  };
  if (variant === 'danger') return {
    background: pressed ? '#a12720' : hovered ? '#d4362b' : '#c8342a',
    color: '#fff',
    borderColor: 'transparent'
  };
  return {};
}
function Button({
  variant = 'primary',
  size = 'md',
  disabled = false,
  type = 'button',
  children,
  onClick,
  style: styleProp,
  ...rest
}) {
  const [hovered, setHovered] = useState(false);
  const [pressed, setPressed] = useState(false);
  return /*#__PURE__*/React.createElement("button", _extends({
    type: type,
    disabled: disabled,
    onClick: onClick,
    onMouseEnter: () => !disabled && setHovered(true),
    onMouseLeave: () => {
      setHovered(false);
      setPressed(false);
    },
    onMouseDown: () => !disabled && setPressed(true),
    onMouseUp: () => setPressed(false),
    style: {
      display: 'inline-flex',
      alignItems: 'center',
      justifyContent: 'center',
      gap: '6px',
      fontFamily: 'var(--font-ui)',
      fontWeight: 500,
      letterSpacing: '0.01em',
      borderRadius: 'var(--radius-md)',
      border: '1px solid transparent',
      cursor: disabled ? 'not-allowed' : 'pointer',
      opacity: disabled ? 0.45 : 1,
      transition: 'var(--transition-colors)',
      whiteSpace: 'nowrap',
      lineHeight: 1,
      userSelect: 'none',
      ...SIZES[size],
      ...getVariantStyle(variant, hovered && !disabled, pressed && !disabled),
      ...styleProp
    }
  }, rest), children);
}
Object.assign(__ds_scope, { Button });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/core/Button.jsx", error: String((e && e.message) || e) }); }

// components/core/Card.jsx
try { (() => {
function _extends() { return _extends = Object.assign ? Object.assign.bind() : function (n) { for (var e = 1; e < arguments.length; e++) { var t = arguments[e]; for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]); } return n; }, _extends.apply(null, arguments); }
const {
  useState
} = React;
function Card({
  children,
  padding = 'md',
  shadow = 'xs',
  border = true,
  onClick,
  as: Tag = 'div',
  style: styleProp,
  ...rest
}) {
  const [hovered, setHovered] = useState(false);
  const isInteractive = typeof onClick === 'function';
  const paddingSizes = {
    none: '0',
    sm: 'var(--space-4)',
    md: 'var(--space-5) var(--space-6)',
    lg: 'var(--space-6) var(--space-8)'
  };
  const shadows = {
    none: 'none',
    xs: 'var(--shadow-xs)',
    sm: 'var(--shadow-sm)',
    md: 'var(--shadow-md)'
  };
  return /*#__PURE__*/React.createElement(Tag, _extends({
    onClick: onClick,
    onMouseEnter: () => isInteractive && setHovered(true),
    onMouseLeave: () => isInteractive && setHovered(false),
    style: {
      background: 'var(--surface-raised)',
      borderRadius: 'var(--radius-xl)',
      border: border ? `1px solid ${hovered && isInteractive ? 'var(--border-strong)' : 'var(--border-ui)'}` : 'none',
      boxShadow: shadows[shadow] || shadows.xs,
      padding: paddingSizes[padding] || paddingSizes.md,
      cursor: isInteractive ? 'pointer' : 'default',
      transition: 'border-color var(--duration-base) var(--ease-standard)',
      ...styleProp
    }
  }, rest), children);
}
Object.assign(__ds_scope, { Card });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/core/Card.jsx", error: String((e && e.message) || e) }); }

// components/core/Tag.jsx
try { (() => {
const {
  useState
} = React;
function Tag({
  children,
  onRemove,
  variant = 'default',
  disabled = false
}) {
  const [hovered, setHovered] = useState(false);
  const variantStyles = {
    default: {
      background: 'var(--ink-50)',
      color: 'var(--text-primary)',
      border: '1px solid var(--ink-200)'
    },
    gold: {
      background: 'var(--gold-50)',
      color: 'var(--gold-800)',
      border: '1px solid var(--gold-200)'
    },
    rigpa: {
      background: 'var(--rigpa-50)',
      color: 'var(--rigpa-800)',
      border: '1px solid var(--rigpa-200)'
    },
    ma: {
      background: 'var(--ma-50)',
      color: 'var(--ma-800)',
      border: '1px solid var(--ma-200)'
    }
  };
  return /*#__PURE__*/React.createElement("span", {
    style: {
      display: 'inline-flex',
      alignItems: 'center',
      gap: '5px',
      padding: '3px 8px',
      borderRadius: 'var(--radius-full)',
      fontFamily: 'var(--font-ui)',
      fontSize: 'var(--text-xs)',
      fontWeight: 400,
      lineHeight: 1.4,
      whiteSpace: 'nowrap',
      ...(variantStyles[variant] || variantStyles.default)
    }
  }, children, onRemove && !disabled && /*#__PURE__*/React.createElement("button", {
    onClick: e => {
      e.stopPropagation();
      onRemove();
    },
    onMouseEnter: () => setHovered(true),
    onMouseLeave: () => setHovered(false),
    style: {
      display: 'inline-flex',
      alignItems: 'center',
      justifyContent: 'center',
      width: '14px',
      height: '14px',
      padding: 0,
      margin: 0,
      background: hovered ? 'var(--ink-200)' : 'transparent',
      border: 'none',
      borderRadius: '50%',
      cursor: 'pointer',
      color: 'currentColor',
      transition: 'background 120ms',
      flexShrink: 0
    },
    "aria-label": "Remove"
  }, /*#__PURE__*/React.createElement("svg", {
    width: "8",
    height: "8",
    viewBox: "0 0 8 8",
    fill: "none"
  }, /*#__PURE__*/React.createElement("path", {
    d: "M1 1l6 6M7 1L1 7",
    stroke: "currentColor",
    strokeWidth: "1.3",
    strokeLinecap: "round"
  }))));
}
Object.assign(__ds_scope, { Tag });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/core/Tag.jsx", error: String((e && e.message) || e) }); }

// components/feedback/Dialog.jsx
try { (() => {
function Dialog({
  open = false,
  onClose,
  title,
  children,
  footer,
  size = 'md'
}) {
  if (!open) return null;
  const maxWidths = {
    sm: '400px',
    md: '560px',
    lg: '760px',
    xl: '960px'
  };
  return /*#__PURE__*/React.createElement("div", {
    onClick: e => {
      if (e.target === e.currentTarget) onClose?.();
    },
    style: {
      position: 'fixed',
      inset: 0,
      zIndex: 1000,
      background: 'rgba(13, 12, 11, 0.48)',
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'center',
      padding: 'var(--space-6)',
      backdropFilter: 'blur(2px)'
    }
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      background: 'var(--surface-raised)',
      borderRadius: 'var(--radius-2xl)',
      boxShadow: 'var(--shadow-xl)',
      width: '100%',
      maxWidth: maxWidths[size] || maxWidths.md,
      border: '1px solid var(--border-ui)',
      maxHeight: '90vh',
      display: 'flex',
      flexDirection: 'column'
    }
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'space-between',
      padding: 'var(--space-5) var(--space-6)',
      borderBottom: '1px solid var(--border-subtle)',
      flexShrink: 0
    }
  }, /*#__PURE__*/React.createElement("h2", {
    style: {
      fontFamily: 'var(--font-display)',
      fontSize: 'var(--text-xl)',
      fontWeight: 500,
      color: 'var(--text-primary)',
      margin: 0,
      lineHeight: 'var(--leading-snug)'
    }
  }, title), onClose && /*#__PURE__*/React.createElement("button", {
    onClick: onClose,
    style: {
      background: 'none',
      border: 'none',
      cursor: 'pointer',
      padding: '4px',
      color: 'var(--text-tertiary)',
      borderRadius: 'var(--radius-sm)',
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'center',
      marginLeft: 'var(--space-3)'
    }
  }, /*#__PURE__*/React.createElement("svg", {
    width: "16",
    height: "16",
    viewBox: "0 0 16 16",
    fill: "none"
  }, /*#__PURE__*/React.createElement("path", {
    d: "M3 3l10 10M13 3L3 13",
    stroke: "currentColor",
    strokeWidth: "1.4",
    strokeLinecap: "round"
  })))), /*#__PURE__*/React.createElement("div", {
    style: {
      padding: 'var(--space-5) var(--space-6)',
      overflowY: 'auto',
      flex: 1
    }
  }, children), footer && /*#__PURE__*/React.createElement("div", {
    style: {
      padding: 'var(--space-4) var(--space-6)',
      borderTop: '1px solid var(--border-subtle)',
      display: 'flex',
      justifyContent: 'flex-end',
      gap: 'var(--space-3)',
      flexShrink: 0
    }
  }, footer)));
}
Object.assign(__ds_scope, { Dialog });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/feedback/Dialog.jsx", error: String((e && e.message) || e) }); }

// components/feedback/Toast.jsx
try { (() => {
const VARIANTS = {
  info: {
    bg: 'var(--ink-900)',
    fg: 'var(--ink-50)',
    dot: 'var(--ink-400)'
  },
  success: {
    bg: 'var(--verified-fg)',
    fg: '#fff',
    dot: '#a8d8b2'
  },
  error: {
    bg: '#c8342a',
    fg: '#fff',
    dot: '#f0a0a0'
  },
  warning: {
    bg: 'var(--gold-800)',
    fg: '#fff',
    dot: 'var(--gold-200)'
  }
};
function Toast({
  message,
  type = 'info',
  detail,
  onDismiss
}) {
  const s = VARIANTS[type] || VARIANTS.info;
  return /*#__PURE__*/React.createElement("div", {
    style: {
      display: 'inline-flex',
      alignItems: 'flex-start',
      gap: 'var(--space-3)',
      padding: 'var(--space-3) var(--space-4)',
      background: s.bg,
      color: s.fg,
      borderRadius: 'var(--radius-xl)',
      boxShadow: 'var(--shadow-xl)',
      fontFamily: 'var(--font-ui)',
      maxWidth: '440px',
      minWidth: '240px',
      border: '1px solid rgba(255,255,255,0.1)'
    }
  }, /*#__PURE__*/React.createElement("span", {
    style: {
      width: '6px',
      height: '6px',
      borderRadius: '50%',
      background: s.dot,
      flexShrink: 0,
      marginTop: '5px'
    }
  }), /*#__PURE__*/React.createElement("div", {
    style: {
      flex: 1,
      minWidth: 0
    }
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      fontSize: 'var(--text-sm)',
      fontWeight: 500,
      lineHeight: 1.4
    }
  }, message), detail && /*#__PURE__*/React.createElement("div", {
    style: {
      fontSize: 'var(--text-xs)',
      opacity: 0.75,
      marginTop: '2px',
      lineHeight: 1.4
    }
  }, detail)), onDismiss && /*#__PURE__*/React.createElement("button", {
    onClick: onDismiss,
    style: {
      background: 'none',
      border: 'none',
      cursor: 'pointer',
      color: 'inherit',
      opacity: 0.65,
      padding: '1px',
      flexShrink: 0,
      display: 'flex',
      alignItems: 'center',
      marginTop: '2px'
    }
  }, /*#__PURE__*/React.createElement("svg", {
    width: "12",
    height: "12",
    viewBox: "0 0 12 12",
    fill: "none"
  }, /*#__PURE__*/React.createElement("path", {
    d: "M2 2l8 8M10 2L2 10",
    stroke: "currentColor",
    strokeWidth: "1.4",
    strokeLinecap: "round"
  }))));
}
Object.assign(__ds_scope, { Toast });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/feedback/Toast.jsx", error: String((e && e.message) || e) }); }

// components/forms/Checkbox.jsx
try { (() => {
const {
  useState
} = React;
function Checkbox({
  checked = false,
  onChange,
  label,
  disabled = false,
  id
}) {
  const [focused, setFocused] = useState(false);
  const cbId = id || (label ? `cb-${label.toLowerCase().replace(/\s+/g, '-')}` : undefined);
  return /*#__PURE__*/React.createElement("label", {
    htmlFor: cbId,
    style: {
      display: 'inline-flex',
      alignItems: 'flex-start',
      gap: 'var(--space-2)',
      cursor: disabled ? 'not-allowed' : 'pointer',
      opacity: disabled ? 0.5 : 1,
      userSelect: 'none'
    }
  }, /*#__PURE__*/React.createElement("span", {
    style: {
      position: 'relative',
      display: 'flex',
      flexShrink: 0,
      marginTop: '2px'
    }
  }, /*#__PURE__*/React.createElement("input", {
    id: cbId,
    type: "checkbox",
    checked: checked,
    onChange: onChange,
    disabled: disabled,
    onFocus: () => setFocused(true),
    onBlur: () => setFocused(false),
    style: {
      position: 'absolute',
      opacity: 0,
      width: '100%',
      height: '100%',
      cursor: 'inherit',
      margin: 0
    }
  }), /*#__PURE__*/React.createElement("span", {
    style: {
      width: '16px',
      height: '16px',
      borderRadius: 'var(--radius-xs)',
      border: `1px solid ${focused ? 'var(--border-focus)' : checked ? 'var(--gold-500)' : 'var(--border-ui)'}`,
      background: checked ? 'var(--gold-500)' : 'var(--surface-raised)',
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'center',
      boxShadow: focused ? 'var(--shadow-focus-gold)' : 'none',
      transition: 'all var(--duration-fast) var(--ease-standard)',
      flexShrink: 0
    }
  }, checked && /*#__PURE__*/React.createElement("svg", {
    width: "10",
    height: "8",
    viewBox: "0 0 10 8",
    fill: "none"
  }, /*#__PURE__*/React.createElement("path", {
    d: "M1 4l3 3 5-6",
    stroke: "var(--action-fg)",
    strokeWidth: "1.5",
    strokeLinecap: "round",
    strokeLinejoin: "round"
  })))), label && /*#__PURE__*/React.createElement("span", {
    style: {
      fontFamily: 'var(--font-ui)',
      fontSize: 'var(--text-sm)',
      color: 'var(--text-primary)',
      lineHeight: 1.5
    }
  }, label));
}
Object.assign(__ds_scope, { Checkbox });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/forms/Checkbox.jsx", error: String((e && e.message) || e) }); }

// components/forms/Input.jsx
try { (() => {
function _extends() { return _extends = Object.assign ? Object.assign.bind() : function (n) { for (var e = 1; e < arguments.length; e++) { var t = arguments[e]; for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]); } return n; }, _extends.apply(null, arguments); }
const {
  useState
} = React;
function Input({
  label,
  value,
  onChange,
  placeholder,
  error,
  helper,
  disabled = false,
  size = 'md',
  prefix,
  suffix,
  type = 'text',
  id,
  style: styleProp,
  ...rest
}) {
  const [focused, setFocused] = useState(false);
  const inputId = id || (label ? `input-${label.toLowerCase().replace(/\s+/g, '-')}` : undefined);
  return /*#__PURE__*/React.createElement("div", {
    style: {
      display: 'flex',
      flexDirection: 'column',
      gap: 'var(--space-1-5)',
      ...styleProp
    }
  }, label && /*#__PURE__*/React.createElement("label", {
    htmlFor: inputId,
    style: {
      fontFamily: 'var(--font-ui)',
      fontSize: 'var(--text-sm)',
      fontWeight: 500,
      color: error ? '#c8342a' : 'var(--text-primary)'
    }
  }, label), /*#__PURE__*/React.createElement("div", {
    style: {
      position: 'relative',
      display: 'flex',
      alignItems: 'center'
    }
  }, prefix && /*#__PURE__*/React.createElement("span", {
    style: {
      position: 'absolute',
      left: '10px',
      color: 'var(--text-tertiary)',
      display: 'flex',
      alignItems: 'center',
      pointerEvents: 'none'
    }
  }, prefix), /*#__PURE__*/React.createElement("input", _extends({
    id: inputId,
    type: type,
    value: value,
    onChange: onChange,
    placeholder: placeholder,
    disabled: disabled,
    onFocus: () => setFocused(true),
    onBlur: () => setFocused(false),
    style: {
      width: '100%',
      boxSizing: 'border-box',
      fontFamily: 'var(--font-ui)',
      fontSize: size === 'sm' ? 'var(--text-xs)' : 'var(--text-sm)',
      color: 'var(--text-primary)',
      background: disabled ? 'var(--surface-overlay)' : 'var(--surface-raised)',
      border: `1px solid ${error ? '#c8342a' : focused ? 'var(--border-focus)' : 'var(--border-ui)'}`,
      borderRadius: 'var(--radius-md)',
      padding: size === 'sm' ? '5px 10px' : '8px 12px',
      paddingLeft: prefix ? '32px' : undefined,
      paddingRight: suffix ? '32px' : undefined,
      outline: 'none',
      boxShadow: focused ? 'var(--shadow-focus-gold)' : 'var(--shadow-inset)',
      transition: 'border-color var(--duration-fast) var(--ease-standard), box-shadow var(--duration-fast) var(--ease-standard)',
      cursor: disabled ? 'not-allowed' : 'text',
      opacity: disabled ? 0.6 : 1
    }
  }, rest)), suffix && /*#__PURE__*/React.createElement("span", {
    style: {
      position: 'absolute',
      right: '10px',
      color: 'var(--text-tertiary)',
      display: 'flex',
      alignItems: 'center',
      pointerEvents: 'none'
    }
  }, suffix)), (error || helper) && /*#__PURE__*/React.createElement("span", {
    style: {
      fontFamily: 'var(--font-ui)',
      fontSize: 'var(--text-xs)',
      color: error ? '#c8342a' : 'var(--text-tertiary)'
    }
  }, error || helper));
}
Object.assign(__ds_scope, { Input });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/forms/Input.jsx", error: String((e && e.message) || e) }); }

// components/forms/Radio.jsx
try { (() => {
const {
  useState
} = React;
function Radio({
  checked = false,
  onChange,
  label,
  disabled = false,
  name,
  value,
  id
}) {
  const [focused, setFocused] = useState(false);
  const radioId = id || (label ? `radio-${label.toLowerCase().replace(/\s+/g, '-')}` : undefined);
  return /*#__PURE__*/React.createElement("label", {
    htmlFor: radioId,
    style: {
      display: 'inline-flex',
      alignItems: 'flex-start',
      gap: 'var(--space-2)',
      cursor: disabled ? 'not-allowed' : 'pointer',
      opacity: disabled ? 0.5 : 1,
      userSelect: 'none'
    }
  }, /*#__PURE__*/React.createElement("span", {
    style: {
      position: 'relative',
      display: 'flex',
      flexShrink: 0,
      marginTop: '2px'
    }
  }, /*#__PURE__*/React.createElement("input", {
    id: radioId,
    type: "radio",
    checked: checked,
    onChange: onChange,
    name: name,
    value: value,
    disabled: disabled,
    onFocus: () => setFocused(true),
    onBlur: () => setFocused(false),
    style: {
      position: 'absolute',
      opacity: 0,
      width: '100%',
      height: '100%',
      cursor: 'inherit',
      margin: 0
    }
  }), /*#__PURE__*/React.createElement("span", {
    style: {
      width: '16px',
      height: '16px',
      borderRadius: '50%',
      border: `1px solid ${focused ? 'var(--border-focus)' : checked ? 'var(--gold-500)' : 'var(--border-ui)'}`,
      background: 'var(--surface-raised)',
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'center',
      boxShadow: focused ? 'var(--shadow-focus-gold)' : 'none',
      transition: 'all var(--duration-fast) var(--ease-standard)',
      flexShrink: 0
    }
  }, checked && /*#__PURE__*/React.createElement("span", {
    style: {
      width: '7px',
      height: '7px',
      borderRadius: '50%',
      background: 'var(--gold-500)'
    }
  }))), label && /*#__PURE__*/React.createElement("span", {
    style: {
      fontFamily: 'var(--font-ui)',
      fontSize: 'var(--text-sm)',
      color: 'var(--text-primary)',
      lineHeight: 1.5
    }
  }, label));
}
Object.assign(__ds_scope, { Radio });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/forms/Radio.jsx", error: String((e && e.message) || e) }); }

// components/forms/Select.jsx
try { (() => {
function _extends() { return _extends = Object.assign ? Object.assign.bind() : function (n) { for (var e = 1; e < arguments.length; e++) { var t = arguments[e]; for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]); } return n; }, _extends.apply(null, arguments); }
const {
  useState
} = React;
function Select({
  label,
  value,
  onChange,
  options = [],
  error,
  helper,
  disabled = false,
  placeholder,
  id,
  style: styleProp,
  ...rest
}) {
  const [focused, setFocused] = useState(false);
  const selectId = id || (label ? `sel-${label.toLowerCase().replace(/\s+/g, '-')}` : undefined);
  return /*#__PURE__*/React.createElement("div", {
    style: {
      display: 'flex',
      flexDirection: 'column',
      gap: 'var(--space-1-5)',
      ...styleProp
    }
  }, label && /*#__PURE__*/React.createElement("label", {
    htmlFor: selectId,
    style: {
      fontFamily: 'var(--font-ui)',
      fontSize: 'var(--text-sm)',
      fontWeight: 500,
      color: error ? '#c8342a' : 'var(--text-primary)'
    }
  }, label), /*#__PURE__*/React.createElement("div", {
    style: {
      position: 'relative'
    }
  }, /*#__PURE__*/React.createElement("select", _extends({
    id: selectId,
    value: value,
    onChange: onChange,
    disabled: disabled,
    onFocus: () => setFocused(true),
    onBlur: () => setFocused(false),
    style: {
      width: '100%',
      appearance: 'none',
      boxSizing: 'border-box',
      fontFamily: 'var(--font-ui)',
      fontSize: 'var(--text-sm)',
      color: value ? 'var(--text-primary)' : 'var(--text-tertiary)',
      background: disabled ? 'var(--surface-overlay)' : 'var(--surface-raised)',
      border: `1px solid ${error ? '#c8342a' : focused ? 'var(--border-focus)' : 'var(--border-ui)'}`,
      borderRadius: 'var(--radius-md)',
      padding: '8px 32px 8px 12px',
      outline: 'none',
      cursor: disabled ? 'not-allowed' : 'pointer',
      boxShadow: focused ? 'var(--shadow-focus-gold)' : 'var(--shadow-inset)',
      transition: 'border-color var(--duration-fast) var(--ease-standard), box-shadow var(--duration-fast) var(--ease-standard)',
      opacity: disabled ? 0.6 : 1
    }
  }, rest), placeholder && /*#__PURE__*/React.createElement("option", {
    value: "",
    disabled: true,
    hidden: true
  }, placeholder), options.map(opt => {
    const val = typeof opt === 'object' ? opt.value : opt;
    const lbl = typeof opt === 'object' ? opt.label : opt;
    return /*#__PURE__*/React.createElement("option", {
      key: val,
      value: val
    }, lbl);
  })), /*#__PURE__*/React.createElement("span", {
    style: {
      position: 'absolute',
      right: '10px',
      top: '50%',
      transform: 'translateY(-50%)',
      pointerEvents: 'none',
      color: 'var(--text-tertiary)'
    }
  }, /*#__PURE__*/React.createElement("svg", {
    width: "12",
    height: "12",
    viewBox: "0 0 12 12",
    fill: "none"
  }, /*#__PURE__*/React.createElement("path", {
    d: "M2 4l4 4 4-4",
    stroke: "currentColor",
    strokeWidth: "1.3",
    strokeLinecap: "round",
    strokeLinejoin: "round"
  })))), (error || helper) && /*#__PURE__*/React.createElement("span", {
    style: {
      fontFamily: 'var(--font-ui)',
      fontSize: 'var(--text-xs)',
      color: error ? '#c8342a' : 'var(--text-tertiary)'
    }
  }, error || helper));
}
Object.assign(__ds_scope, { Select });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/forms/Select.jsx", error: String((e && e.message) || e) }); }

// components/forms/Switch.jsx
try { (() => {
const {
  useState
} = React;
function Switch({
  checked = false,
  onChange,
  label,
  disabled = false
}) {
  const [focused, setFocused] = useState(false);
  return /*#__PURE__*/React.createElement("label", {
    style: {
      display: 'inline-flex',
      alignItems: 'center',
      gap: 'var(--space-3)',
      cursor: disabled ? 'not-allowed' : 'pointer',
      opacity: disabled ? 0.5 : 1,
      userSelect: 'none'
    }
  }, /*#__PURE__*/React.createElement("span", {
    style: {
      position: 'relative',
      display: 'flex',
      flexShrink: 0
    }
  }, /*#__PURE__*/React.createElement("input", {
    type: "checkbox",
    checked: checked,
    onChange: onChange,
    disabled: disabled,
    onFocus: () => setFocused(true),
    onBlur: () => setFocused(false),
    style: {
      position: 'absolute',
      opacity: 0,
      width: '100%',
      height: '100%',
      cursor: 'inherit',
      margin: 0
    }
  }), /*#__PURE__*/React.createElement("span", {
    style: {
      width: '36px',
      height: '20px',
      borderRadius: 'var(--radius-full)',
      background: checked ? 'var(--gold-500)' : 'var(--ink-200)',
      boxShadow: focused ? 'var(--shadow-focus-gold)' : 'none',
      transition: 'background var(--duration-base) var(--ease-standard), box-shadow var(--duration-fast) var(--ease-standard)',
      display: 'block',
      position: 'relative',
      flexShrink: 0
    }
  }, /*#__PURE__*/React.createElement("span", {
    style: {
      position: 'absolute',
      top: '3px',
      left: checked ? '19px' : '3px',
      width: '14px',
      height: '14px',
      borderRadius: '50%',
      background: 'white',
      boxShadow: 'var(--shadow-sm)',
      transition: 'left var(--duration-base) var(--ease-standard)'
    }
  }))), label && /*#__PURE__*/React.createElement("span", {
    style: {
      fontFamily: 'var(--font-ui)',
      fontSize: 'var(--text-sm)',
      color: 'var(--text-primary)'
    }
  }, label));
}
Object.assign(__ds_scope, { Switch });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/forms/Switch.jsx", error: String((e && e.message) || e) }); }

// components/forms/Textarea.jsx
try { (() => {
function _extends() { return _extends = Object.assign ? Object.assign.bind() : function (n) { for (var e = 1; e < arguments.length; e++) { var t = arguments[e]; for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]); } return n; }, _extends.apply(null, arguments); }
const {
  useState
} = React;
function Textarea({
  label,
  value,
  onChange,
  placeholder,
  error,
  helper,
  disabled = false,
  rows = 4,
  id,
  style: styleProp,
  ...rest
}) {
  const [focused, setFocused] = useState(false);
  const areaId = id || (label ? `ta-${label.toLowerCase().replace(/\s+/g, '-')}` : undefined);
  return /*#__PURE__*/React.createElement("div", {
    style: {
      display: 'flex',
      flexDirection: 'column',
      gap: 'var(--space-1-5)',
      ...styleProp
    }
  }, label && /*#__PURE__*/React.createElement("label", {
    htmlFor: areaId,
    style: {
      fontFamily: 'var(--font-ui)',
      fontSize: 'var(--text-sm)',
      fontWeight: 500,
      color: error ? '#c8342a' : 'var(--text-primary)'
    }
  }, label), /*#__PURE__*/React.createElement("textarea", _extends({
    id: areaId,
    value: value,
    onChange: onChange,
    placeholder: placeholder,
    disabled: disabled,
    rows: rows,
    onFocus: () => setFocused(true),
    onBlur: () => setFocused(false),
    style: {
      width: '100%',
      boxSizing: 'border-box',
      fontFamily: 'var(--font-serif)',
      fontSize: 'var(--text-sm)',
      color: 'var(--text-primary)',
      background: disabled ? 'var(--surface-overlay)' : 'var(--surface-raised)',
      border: `1px solid ${error ? '#c8342a' : focused ? 'var(--border-focus)' : 'var(--border-ui)'}`,
      borderRadius: 'var(--radius-md)',
      padding: '8px 12px',
      outline: 'none',
      resize: 'vertical',
      boxShadow: focused ? 'var(--shadow-focus-gold)' : 'var(--shadow-inset)',
      lineHeight: 'var(--leading-relaxed)',
      transition: 'border-color var(--duration-fast) var(--ease-standard), box-shadow var(--duration-fast) var(--ease-standard)',
      cursor: disabled ? 'not-allowed' : 'text',
      opacity: disabled ? 0.6 : 1
    }
  }, rest)), (error || helper) && /*#__PURE__*/React.createElement("span", {
    style: {
      fontFamily: 'var(--font-ui)',
      fontSize: 'var(--text-xs)',
      color: error ? '#c8342a' : 'var(--text-tertiary)'
    }
  }, error || helper));
}
Object.assign(__ds_scope, { Textarea });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/forms/Textarea.jsx", error: String((e && e.message) || e) }); }

// components/knowledge/BranchRef.jsx
try { (() => {
function BranchRef({
  branch,
  short = false,
  commit
}) {
  const isMa = branch?.startsWith('ma/');
  const isRigpa = branch?.startsWith('rigpa/');
  const styles = isMa ? {
    background: 'var(--ma-50)',
    color: 'var(--ma-700)',
    border: '1px solid var(--ma-200)'
  } : isRigpa ? {
    background: 'var(--rigpa-50)',
    color: 'var(--rigpa-700)',
    border: '1px solid var(--rigpa-200)'
  } : {
    background: 'var(--ink-50)',
    color: 'var(--ink-600)',
    border: '1px solid var(--ink-200)'
  };
  const prefix = isMa ? 'ma/' : isRigpa ? 'rigpa/' : '';
  const stem = branch?.slice(prefix.length) ?? branch;
  const display = short ? stem : branch;
  return /*#__PURE__*/React.createElement("span", {
    style: {
      display: 'inline-flex',
      alignItems: 'center',
      gap: '5px',
      flexShrink: 0
    }
  }, /*#__PURE__*/React.createElement("span", {
    style: {
      display: 'inline-flex',
      alignItems: 'center',
      padding: '1px 7px',
      borderRadius: 'var(--radius-sm)',
      fontFamily: 'var(--font-mono)',
      fontSize: 'var(--text-xs)',
      fontWeight: 400,
      whiteSpace: 'nowrap',
      lineHeight: 1.6,
      ...styles
    }
  }, display), commit && /*#__PURE__*/React.createElement("span", {
    style: {
      fontFamily: 'var(--font-mono)',
      fontSize: 'var(--text-xs)',
      color: 'var(--text-tertiary)'
    }
  }, "@", commit.slice(0, 7)));
}
Object.assign(__ds_scope, { BranchRef });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/knowledge/BranchRef.jsx", error: String((e && e.message) || e) }); }

// components/knowledge/ClaimMarker.jsx
try { (() => {
const MARKER_STYLES = {
  verified: {
    bg: 'var(--verified-bg)',
    fg: 'var(--verified-fg)',
    border: 'var(--verified-border)'
  },
  inference: {
    bg: 'var(--inference-bg)',
    fg: 'var(--inference-fg)',
    border: 'var(--inference-border)'
  },
  open: {
    bg: 'var(--open-bg)',
    fg: 'var(--open-fg)',
    border: 'var(--open-border)'
  }
};

/**
 * Inline epistemic annotation: [verified], [inference], or [open].
 * Renders in monospace to clearly distinguish from surrounding prose.
 */
function ClaimMarker({
  type = 'open',
  annotation
}) {
  const s = MARKER_STYLES[type] || MARKER_STYLES.open;
  return /*#__PURE__*/React.createElement("span", {
    style: {
      display: 'inline-flex',
      alignItems: 'baseline',
      gap: '3px',
      padding: '1px 6px',
      borderRadius: 'var(--radius-sm)',
      fontFamily: 'var(--font-mono)',
      fontSize: '0.78em',
      /* scales with surrounding text */
      fontWeight: 500,
      lineHeight: 1.6,
      whiteSpace: 'nowrap',
      verticalAlign: 'baseline',
      background: s.bg,
      color: s.fg,
      border: `1px solid ${s.border}`
    }
  }, "[", type, "]", annotation && /*#__PURE__*/React.createElement("span", {
    style: {
      fontFamily: 'var(--font-serif)',
      fontStyle: 'italic',
      fontWeight: 400,
      marginLeft: '3px'
    }
  }, annotation));
}
Object.assign(__ds_scope, { ClaimMarker });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/knowledge/ClaimMarker.jsx", error: String((e && e.message) || e) }); }

// components/knowledge/NodeCard.jsx
try { (() => {
const {
  useState
} = React;
const CLAIM_STYLES = {
  verified: {
    bg: 'var(--verified-bg)',
    fg: 'var(--verified-fg)',
    border: 'var(--verified-border)'
  },
  inference: {
    bg: 'var(--inference-bg)',
    fg: 'var(--inference-fg)',
    border: 'var(--inference-border)'
  },
  open: {
    bg: 'var(--open-bg)',
    fg: 'var(--open-fg)',
    border: 'var(--open-border)'
  }
};
function NodeCard({
  path,
  title,
  excerpt,
  outgoing = 0,
  incoming = 0,
  lineCount,
  lastModified,
  markers = [],
  isOpenQuestion = false,
  onClick,
  style: styleProp
}) {
  const [hovered, setHovered] = useState(false);
  const isInteractive = typeof onClick === 'function';
  return /*#__PURE__*/React.createElement("div", {
    onClick: onClick,
    onMouseEnter: () => isInteractive && setHovered(true),
    onMouseLeave: () => isInteractive && setHovered(false),
    style: {
      background: 'var(--surface-raised)',
      border: `1px solid ${hovered ? 'var(--border-strong)' : 'var(--border-ui)'}`,
      borderRadius: 'var(--radius-xl)',
      padding: 'var(--space-5) var(--space-6)',
      cursor: isInteractive ? 'pointer' : 'default',
      transition: 'border-color var(--duration-fast) var(--ease-standard)',
      boxShadow: 'var(--shadow-xs)',
      ...styleProp
    }
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      fontFamily: 'var(--font-mono)',
      fontSize: 'var(--text-xs)',
      color: 'var(--text-tertiary)',
      marginBottom: 'var(--space-2)',
      display: 'flex',
      alignItems: 'center',
      gap: '4px'
    }
  }, isOpenQuestion && /*#__PURE__*/React.createElement("span", {
    style: {
      color: 'var(--open-fg)',
      fontWeight: 500
    }
  }, "?"), path), /*#__PURE__*/React.createElement("div", {
    style: {
      fontFamily: 'var(--font-display)',
      fontSize: 'var(--text-xl)',
      fontWeight: 500,
      color: 'var(--text-primary)',
      lineHeight: 'var(--leading-snug)',
      marginBottom: excerpt ? 'var(--space-3)' : 'var(--space-4)'
    }
  }, title), excerpt && /*#__PURE__*/React.createElement("div", {
    style: {
      fontFamily: 'var(--font-serif)',
      fontSize: 'var(--text-sm)',
      color: 'var(--text-secondary)',
      lineHeight: 'var(--leading-relaxed)',
      marginBottom: 'var(--space-4)',
      display: '-webkit-box',
      WebkitLineClamp: 3,
      WebkitBoxOrient: 'vertical',
      overflow: 'hidden'
    }
  }, excerpt), markers.length > 0 && /*#__PURE__*/React.createElement("div", {
    style: {
      display: 'flex',
      gap: 'var(--space-1-5)',
      flexWrap: 'wrap',
      marginBottom: 'var(--space-4)'
    }
  }, markers.map((m, i) => {
    const s = CLAIM_STYLES[m] || CLAIM_STYLES.open;
    return /*#__PURE__*/React.createElement("span", {
      key: i,
      style: {
        display: 'inline-flex',
        padding: '1px 6px',
        borderRadius: 'var(--radius-sm)',
        fontFamily: 'var(--font-mono)',
        fontSize: 'var(--text-xs)',
        fontWeight: 500,
        background: s.bg,
        color: s.fg,
        border: `1px solid ${s.border}`
      }
    }, "[", m, "]");
  })), /*#__PURE__*/React.createElement("div", {
    style: {
      display: 'flex',
      gap: 'var(--space-5)',
      alignItems: 'center',
      fontFamily: 'var(--font-ui)',
      fontSize: 'var(--text-xs)',
      color: 'var(--text-tertiary)',
      borderTop: '1px solid var(--border-subtle)',
      paddingTop: 'var(--space-3)',
      marginTop: markers.length === 0 && !excerpt ? 0 : undefined
    }
  }, /*#__PURE__*/React.createElement("span", {
    title: "Outgoing edges",
    style: {
      display: 'flex',
      alignItems: 'center',
      gap: '3px'
    }
  }, /*#__PURE__*/React.createElement("svg", {
    width: "10",
    height: "10",
    viewBox: "0 0 10 10",
    fill: "none"
  }, /*#__PURE__*/React.createElement("path", {
    d: "M2 5h6M6 2.5l3 2.5-3 2.5",
    stroke: "currentColor",
    strokeWidth: "1.2",
    strokeLinecap: "round",
    strokeLinejoin: "round"
  })), outgoing), /*#__PURE__*/React.createElement("span", {
    title: "Incoming edges",
    style: {
      display: 'flex',
      alignItems: 'center',
      gap: '3px'
    }
  }, /*#__PURE__*/React.createElement("svg", {
    width: "10",
    height: "10",
    viewBox: "0 0 10 10",
    fill: "none"
  }, /*#__PURE__*/React.createElement("path", {
    d: "M8 5H2M4 2.5L1 5l3 2.5",
    stroke: "currentColor",
    strokeWidth: "1.2",
    strokeLinecap: "round",
    strokeLinejoin: "round"
  })), incoming), lineCount != null && /*#__PURE__*/React.createElement("span", null, lineCount, " lines"), lastModified && /*#__PURE__*/React.createElement("span", null, lastModified)));
}
Object.assign(__ds_scope, { NodeCard });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/knowledge/NodeCard.jsx", error: String((e && e.message) || e) }); }

// components/knowledge/PhaseTag.jsx
try { (() => {
const PHASE = {
  Investigation: {
    bg: 'var(--phase-investigation-bg)',
    fg: 'var(--phase-investigation-fg)',
    border: 'var(--rigpa-100)'
  },
  Extraction: {
    bg: 'var(--phase-extraction-bg)',
    fg: 'var(--phase-extraction-fg)',
    border: 'var(--gold-200)'
  },
  Synthesis: {
    bg: 'var(--phase-synthesis-bg)',
    fg: 'var(--phase-synthesis-fg)',
    border: 'var(--ma-200)'
  },
  Assessment: {
    bg: 'var(--phase-assessment-bg)',
    fg: 'var(--phase-assessment-fg)',
    border: '#aad4b4'
  }
};
function PhaseTag({
  phase
}) {
  const s = PHASE[phase] || PHASE.Investigation;
  return /*#__PURE__*/React.createElement("span", {
    style: {
      display: 'inline-flex',
      alignItems: 'center',
      padding: '2px 8px',
      borderRadius: 'var(--radius-sm)',
      fontFamily: 'var(--font-ui)',
      fontSize: 'var(--text-xs)',
      fontWeight: 500,
      letterSpacing: 'var(--tracking-wider)',
      textTransform: 'uppercase',
      whiteSpace: 'nowrap',
      background: s.bg,
      color: s.fg,
      border: `1px solid ${s.border}`
    }
  }, phase);
}
Object.assign(__ds_scope, { PhaseTag });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/knowledge/PhaseTag.jsx", error: String((e && e.message) || e) }); }

// components/navigation/Breadcrumb.jsx
try { (() => {
function Breadcrumb({
  items = [],
  mono = false
}) {
  return /*#__PURE__*/React.createElement("nav", {
    "aria-label": "Breadcrumb"
  }, /*#__PURE__*/React.createElement("ol", {
    style: {
      display: 'flex',
      alignItems: 'center',
      flexWrap: 'wrap',
      gap: '0',
      listStyle: 'none',
      margin: 0,
      padding: 0,
      fontFamily: mono ? 'var(--font-mono)' : 'var(--font-ui)',
      fontSize: 'var(--text-xs)'
    }
  }, items.map((item, i) => {
    const isLast = i === items.length - 1;
    return /*#__PURE__*/React.createElement("li", {
      key: i,
      style: {
        display: 'flex',
        alignItems: 'center'
      }
    }, i > 0 && /*#__PURE__*/React.createElement("span", {
      style: {
        margin: '0 var(--space-1)',
        color: 'var(--text-tertiary)',
        userSelect: 'none'
      }
    }, /*#__PURE__*/React.createElement("svg", {
      width: "10",
      height: "10",
      viewBox: "0 0 10 10",
      fill: "none"
    }, /*#__PURE__*/React.createElement("path", {
      d: "M3 2l4 3-4 3",
      stroke: "currentColor",
      strokeWidth: "1.2",
      strokeLinecap: "round",
      strokeLinejoin: "round"
    }))), isLast ? /*#__PURE__*/React.createElement("span", {
      style: {
        color: 'var(--text-primary)',
        fontWeight: 500
      }
    }, item.label) : /*#__PURE__*/React.createElement("a", {
      href: item.href || '#',
      style: {
        color: 'var(--text-tertiary)',
        textDecoration: 'none',
        transition: 'color var(--duration-fast)'
      },
      onMouseEnter: e => e.currentTarget.style.color = 'var(--text-secondary)',
      onMouseLeave: e => e.currentTarget.style.color = 'var(--text-tertiary)'
    }, item.label));
  })));
}
Object.assign(__ds_scope, { Breadcrumb });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/navigation/Breadcrumb.jsx", error: String((e && e.message) || e) }); }

// components/navigation/Tabs.jsx
try { (() => {
function Tabs({
  tabs = [],
  activeTab,
  onChange,
  size = 'md',
  variant = 'underline',
  children
}) {
  return /*#__PURE__*/React.createElement("div", null, /*#__PURE__*/React.createElement("div", {
    role: "tablist",
    style: {
      display: 'flex',
      borderBottom: variant === 'underline' ? '1px solid var(--border-ui)' : 'none',
      gap: variant === 'pill' ? 'var(--space-1)' : 0,
      background: variant === 'pill' ? 'var(--surface-overlay)' : 'transparent',
      borderRadius: variant === 'pill' ? 'var(--radius-lg)' : 0,
      padding: variant === 'pill' ? '3px' : 0
    }
  }, tabs.map(tab => {
    const isActive = tab.id === activeTab;
    const isDisabled = tab.disabled;
    if (variant === 'pill') {
      return /*#__PURE__*/React.createElement("button", {
        key: tab.id,
        role: "tab",
        "aria-selected": isActive,
        onClick: () => !isDisabled && onChange && onChange(tab.id),
        style: {
          padding: size === 'sm' ? '4px 12px' : '5px 14px',
          fontFamily: 'var(--font-ui)',
          fontSize: size === 'sm' ? 'var(--text-xs)' : 'var(--text-sm)',
          fontWeight: isActive ? 500 : 400,
          color: isActive ? 'var(--text-primary)' : 'var(--text-secondary)',
          background: isActive ? 'var(--surface-raised)' : 'transparent',
          border: 'none',
          borderRadius: 'var(--radius-md)',
          cursor: isDisabled ? 'not-allowed' : 'pointer',
          opacity: isDisabled ? 0.4 : 1,
          boxShadow: isActive ? 'var(--shadow-sm)' : 'none',
          transition: 'all var(--duration-fast) var(--ease-standard)',
          whiteSpace: 'nowrap'
        }
      }, tab.label);
    }
    return /*#__PURE__*/React.createElement("button", {
      key: tab.id,
      role: "tab",
      "aria-selected": isActive,
      onClick: () => !isDisabled && onChange && onChange(tab.id),
      style: {
        padding: size === 'sm' ? '6px 12px' : '8px 16px',
        fontFamily: 'var(--font-ui)',
        fontSize: size === 'sm' ? 'var(--text-xs)' : 'var(--text-sm)',
        fontWeight: isActive ? 500 : 400,
        color: isActive ? 'var(--text-primary)' : 'var(--text-secondary)',
        background: 'transparent',
        border: 'none',
        borderBottom: `2px solid ${isActive ? 'var(--gold-500)' : 'transparent'}`,
        marginBottom: '-1px',
        cursor: isDisabled ? 'not-allowed' : 'pointer',
        opacity: isDisabled ? 0.4 : 1,
        transition: 'color var(--duration-fast) var(--ease-standard), border-color var(--duration-fast) var(--ease-standard)',
        whiteSpace: 'nowrap',
        display: 'flex',
        alignItems: 'center',
        gap: '6px'
      }
    }, tab.label, tab.count !== undefined && /*#__PURE__*/React.createElement("span", {
      style: {
        background: isActive ? 'var(--gold-100)' : 'var(--ink-100)',
        color: isActive ? 'var(--gold-800)' : 'var(--text-tertiary)',
        padding: '0 5px',
        borderRadius: 'var(--radius-full)',
        fontSize: 'var(--text-2xs)',
        fontWeight: 500,
        lineHeight: '16px'
      }
    }, tab.count));
  })), children && /*#__PURE__*/React.createElement("div", {
    style: {
      paddingTop: 'var(--space-4)'
    }
  }, children));
}
Object.assign(__ds_scope, { Tabs });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/navigation/Tabs.jsx", error: String((e && e.message) || e) }); }

// components/navigation/Tooltip.jsx
try { (() => {
const {
  useState,
  useRef
} = React;
const POSITION_STYLE = {
  top: {
    bottom: 'calc(100% + 7px)',
    left: '50%',
    transform: 'translateX(-50%)'
  },
  bottom: {
    top: 'calc(100% + 7px)',
    left: '50%',
    transform: 'translateX(-50%)'
  },
  left: {
    right: 'calc(100% + 7px)',
    top: '50%',
    transform: 'translateY(-50%)'
  },
  right: {
    left: 'calc(100% + 7px)',
    top: '50%',
    transform: 'translateY(-50%)'
  }
};
function Tooltip({
  content,
  children,
  position = 'top',
  delay = 400
}) {
  const [visible, setVisible] = useState(false);
  const timer = useRef(null);
  function show() {
    clearTimeout(timer.current);
    timer.current = setTimeout(() => setVisible(true), delay);
  }
  function hide() {
    clearTimeout(timer.current);
    setVisible(false);
  }
  return /*#__PURE__*/React.createElement("span", {
    style: {
      position: 'relative',
      display: 'inline-flex'
    },
    onMouseEnter: show,
    onMouseLeave: hide,
    onFocus: show,
    onBlur: hide
  }, children, visible && content && /*#__PURE__*/React.createElement("span", {
    style: {
      position: 'absolute',
      zIndex: 1000,
      background: 'var(--ink-900)',
      color: 'var(--text-inverse)',
      padding: '5px 9px',
      borderRadius: 'var(--radius-md)',
      fontFamily: 'var(--font-ui)',
      fontSize: 'var(--text-xs)',
      fontWeight: 400,
      whiteSpace: 'nowrap',
      boxShadow: 'var(--shadow-md)',
      pointerEvents: 'none',
      lineHeight: 1.4,
      ...(POSITION_STYLE[position] || POSITION_STYLE.top)
    }
  }, content));
}
Object.assign(__ds_scope, { Tooltip });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/navigation/Tooltip.jsx", error: String((e && e.message) || e) }); }

__ds_ns.Avatar = __ds_scope.Avatar;

__ds_ns.Badge = __ds_scope.Badge;

__ds_ns.Button = __ds_scope.Button;

__ds_ns.Card = __ds_scope.Card;

__ds_ns.Tag = __ds_scope.Tag;

__ds_ns.Dialog = __ds_scope.Dialog;

__ds_ns.Toast = __ds_scope.Toast;

__ds_ns.Checkbox = __ds_scope.Checkbox;

__ds_ns.Input = __ds_scope.Input;

__ds_ns.Radio = __ds_scope.Radio;

__ds_ns.Select = __ds_scope.Select;

__ds_ns.Switch = __ds_scope.Switch;

__ds_ns.Textarea = __ds_scope.Textarea;

__ds_ns.BranchRef = __ds_scope.BranchRef;

__ds_ns.ClaimMarker = __ds_scope.ClaimMarker;

__ds_ns.NodeCard = __ds_scope.NodeCard;

__ds_ns.PhaseTag = __ds_scope.PhaseTag;

__ds_ns.Breadcrumb = __ds_scope.Breadcrumb;

__ds_ns.Tabs = __ds_scope.Tabs;

__ds_ns.Tooltip = __ds_scope.Tooltip;

})();

