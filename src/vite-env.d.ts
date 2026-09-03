/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_APP_TITLE?: string
  readonly VITE_APP_ENV?: string
  readonly VITE_CHAT_ASSISTANT?: string
  readonly VITE_FLUFFY_OSS_BASE_URL?: string
  readonly VITE_FLUFFY_OSS_APP_ID?: string
  readonly VITE_FLUFFY_OSS_SECRET?: string
  readonly VITE_FLUFFY_OSS_PROXY_TARGET?: string
  readonly VITE_FLUFFY_LOG_APP_ID?: string
  readonly VITE_FLUFFY_LOG_BASE_URL?: string
  readonly VITE_FLUFFY_LOG_CREDENTIAL?: string
  readonly VITE_FLUFFY_LOG_PROXY_TARGET?: string
}

interface ImportMeta {
  readonly env: ImportMetaEnv
}
