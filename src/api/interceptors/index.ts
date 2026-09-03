import axios, { AxiosHeaders, type AxiosInstance, type AxiosResponse, type InternalAxiosRequestConfig } from 'axios'
import { ApiError, type ApiEnvelope } from '../contracts'
import { clearAccessToken, getAccessToken } from '../token'

function createApiError(message: string, options: ConstructorParameters<typeof ApiError>[1] = {}) {
  return new ApiError(message, options)
}

function attachAccessToken(config: InternalAxiosRequestConfig) {
  const token = getAccessToken()
  if (!token) return config

  const headers = AxiosHeaders.from(config.headers)
  if (!headers.has('Authorization')) headers.set('Authorization', `Bearer ${token}`)
  config.headers = headers
  return config
}

function unwrapEnvelope(response: AxiosResponse<ApiEnvelope<unknown>>) {
  const envelope = response.data
  if (envelope.code === 200) return envelope.data

  throw createApiError(envelope.message || '请求未成功完成', {
    code: envelope.code,
    status: response.status,
    data: envelope.data,
  })
}

function normalizeRequestError(error: unknown) {
  if (error instanceof ApiError) return error

  if (axios.isCancel(error)) return createApiError('请求已取消', { cause: error })

  if (axios.isAxiosError(error)) {
    const status = error.response?.status
    if (status === 401) clearAccessToken()

    if (error.code === 'ECONNABORTED') return createApiError('请求超时，请稍后重试', { status, data: error.response?.data, cause: error })
    if (!error.response) return createApiError('网络连接异常，请检查网络后重试', { cause: error })

    const data = error.response.data as Partial<ApiEnvelope<unknown>> | undefined
    return createApiError(data?.message || '请求失败，请稍后重试', {
      code: data?.code,
      status,
      data: error.response.data,
      cause: error,
    })
  }

  return createApiError('请求失败，请稍后重试', { cause: error })
}

export function registerInterceptors(request: AxiosInstance) {
  request.interceptors.request.use(attachAccessToken)
  request.interceptors.response.use(
    unwrapEnvelope as unknown as (response: AxiosResponse) => AxiosResponse,
    (error: unknown) => Promise.reject(normalizeRequestError(error)),
  )
}
