export interface HeaderActions {
  search: boolean
  language: boolean
  theme: boolean
  notifications: boolean
  fullscreen: boolean
  account: boolean
  settings: boolean
  uploadCenter: boolean
}

export const appConfig = {
  name: 'OpenSCP',
  defaultLocale: 'zh-CN' as const,
  defaultDarkMode: true,
  showTabBar: true,
  themeColor: '#0878FE',
  permission: {
    tokens: [] as string[],
    tokenSeparator: '|'
  },
  showNavbar: true,
  showMenu: true,
  menuWidth: 244,
  colorWeak: false,
  documentTitle: true,
  headerActions: {
    search: true,
    language: true,
    theme: true,
    notifications: true,
    fullscreen: true,
    account: false,
    settings: true,
    uploadCenter: false
  } satisfies HeaderActions
}

