/** @type {import('tailwindcss').Config} */
module.exports = {
  content: [
    "./src/**/*.rs",
    "./index.html",
  ],
  darkMode: 'class',
  theme: {
    extend: {
      colors: {
        // Core theme colors — mapped from CSS variables set by applyThemeToCss
        primary: 'var(--color-primary)',
        secondary: 'var(--color-secondary)',
        accent: 'var(--color-accent)',
        bg: {
          DEFAULT: 'var(--color-bg)',
          panel: 'var(--color-bg-panel)',
          element: 'var(--color-bg-element)',
          hover: 'var(--color-bg-hover)',
          selected: 'var(--color-bg-selected)',
        },
        text: {
          DEFAULT: 'var(--color-text)',
          muted: 'var(--color-text-muted)',
          secondary: 'var(--color-text-secondary)',
        },
        border: {
          DEFAULT: 'var(--color-border)',
          active: 'var(--color-border-active)',
          subtle: 'var(--color-border-subtle)',
        },
        error: 'var(--color-error)',
        warning: 'var(--color-warning)',
        success: 'var(--color-success)',
        info: 'var(--color-info)',
        // Computed theme surfaces
        surface: {
          1: 'var(--theme-surface-1)',
          2: 'var(--theme-surface-2)',
          3: 'var(--theme-surface-3)',
          hover: 'var(--theme-surface-hover)',
          elevated: 'var(--theme-elevated)',
          overlay: 'var(--theme-overlay)',
        },
        // Soft semantic colors
        'primary-soft': 'var(--theme-primary-soft)',
        'primary-muted': 'var(--color-primary-muted)',
        'primary-border': 'var(--theme-primary-border)',
        'success-soft': 'var(--theme-success-soft)',
        'error-soft': 'var(--theme-error-soft)',
        'warning-soft': 'var(--theme-warning-soft)',
        'accent-soft': 'var(--theme-accent-soft)',
        // Syntax highlighting
        syntax: {
          keyword: 'var(--color-syntax-keyword)',
          function: 'var(--color-syntax-function)',
          string: 'var(--color-syntax-string)',
          number: 'var(--color-syntax-number)',
          comment: 'var(--color-syntax-comment)',
          operator: 'var(--color-syntax-operator)',
          punctuation: 'var(--color-syntax-punctuation)',
          tag: 'var(--color-syntax-tag)',
          attribute: 'var(--color-syntax-attribute)',
          regex: 'var(--color-syntax-regex)',
        },
      },
      fontFamily: {
        mono: 'var(--font-mono)',
        sans: 'var(--font-sans)',
      },
      fontSize: {
        '2xs': 'var(--font-size-2xs)',
        xs: 'var(--font-size-xs)',
        sm: 'var(--font-size-sm)',
        base: 'var(--font-size-base)',
        lg: 'var(--font-size-lg)',
        xl: 'var(--font-size-xl)',
      },
      spacing: {
        'sp-1': 'var(--space-1)',
        'sp-2': 'var(--space-2)',
        'sp-3': 'var(--space-3)',
        'sp-4': 'var(--space-4)',
        'sp-5': 'var(--space-5)',
        'sp-6': 'var(--space-6)',
        'sp-8': 'var(--space-8)',
        'sp-10': 'var(--space-10)',
        'sp-12': 'var(--space-12)',
        'sidebar': 'var(--sidebar-width)',
        'statusbar': 'var(--status-bar-height)',
        'chat-max': 'var(--chat-max-width)',
      },
      borderRadius: {
        sm: 'var(--radius-sm)',
        DEFAULT: 'var(--radius-md)',
        md: 'var(--radius-md)',
        lg: 'var(--radius-lg)',
        xl: 'var(--radius-xl)',
        full: 'var(--radius-full)',
      },
      transitionDuration: {
        fast: 'var(--transition-fast)',
        base: 'var(--transition-base)',
        slow: 'var(--transition-slow)',
      },
      boxShadow: {
        sm: 'var(--shadow-sm)',
        DEFAULT: 'var(--shadow-md)',
        md: 'var(--shadow-md)',
        lg: 'var(--shadow-lg)',
      },
      backdropBlur: {
        sm: 'var(--glass-blur-sm)',
        DEFAULT: 'var(--glass-blur-md)',
        md: 'var(--glass-blur-md)',
        lg: 'var(--glass-blur-lg)',
      },
      zIndex: {
        sidebar: 'var(--z-sidebar)',
        overlay: 'var(--z-overlay)',
        modal: 'var(--z-modal)',
      },
      animation: {
        'pulse-slow': 'pulse 3s ease-in-out infinite',
        'gradient-mesh': 'gradient-mesh-shift 30s ease-in-out infinite alternate',
        'toast-in': 'toast-slide-in 0.3s ease-out',
        'pill-pulse': 'pill-pulse 2s ease-in-out infinite',
      },
      keyframes: {
        'gradient-mesh-shift': {
          '0%': { backgroundPosition: '0% 50%' },
          '100%': { backgroundPosition: '100% 50%' },
        },
        'toast-slide-in': {
          from: { transform: 'translateY(-100%)', opacity: '0' },
          to: { transform: 'translateY(0)', opacity: '1' },
        },
        'pill-pulse': {
          '0%, 100%': { opacity: '1' },
          '50%': { opacity: '0.6' },
        },
      },
    },
  },
  plugins: [],
}
