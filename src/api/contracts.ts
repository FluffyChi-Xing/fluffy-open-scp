export interface ApiEnvelope<T> {
  code: number
  message?: string
  data: T
}

export interface ApiErrorOptions {
  code?: number
  status?: number
  data?: unknown
  cause?: unknown
}

export class ApiError extends Error {
  code?: number
  status?: number
  data?: unknown

  constructor(message: string, options: ApiErrorOptions = {}) {
    super(message, options.cause === undefined ? undefined : { cause: options.cause })
    this.name = 'ApiError'
    this.code = options.code
    this.status = options.status
    this.data = options.data
  }
}
