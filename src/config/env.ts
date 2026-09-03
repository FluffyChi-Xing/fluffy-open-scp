export interface AppEnv {
  title: string
  env: string
  oss: { baseUrl: string; appId: string; secret: string; proxyTarget: string }
  logTrace: { appId: string; baseUrl: string; credential: string; proxyTarget: string }
}

export function readAppEnv(): AppEnv {
  return {
    title: import.meta.env.VITE_APP_TITLE ?? '',
    env: import.meta.env.VITE_APP_ENV ?? 'development',
    oss: {
      baseUrl: import.meta.env.VITE_FLUFFY_OSS_BASE_URL ?? '',
      appId: import.meta.env.VITE_FLUFFY_OSS_APP_ID ?? '',
      secret: import.meta.env.VITE_FLUFFY_OSS_SECRET ?? '',
      proxyTarget: import.meta.env.VITE_FLUFFY_OSS_PROXY_TARGET ?? ''
    },
    logTrace: {
      appId: import.meta.env.VITE_FLUFFY_LOG_APP_ID ?? '',
      baseUrl: import.meta.env.VITE_FLUFFY_LOG_BASE_URL ?? '',
      credential: import.meta.env.VITE_FLUFFY_LOG_CREDENTIAL ?? '',
      proxyTarget: import.meta.env.VITE_FLUFFY_LOG_PROXY_TARGET ?? ''
    }
  }
}

export const appEnv = readAppEnv()
