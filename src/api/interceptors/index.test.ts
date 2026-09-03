import axios, { AxiosError, AxiosHeaders, type AxiosAdapter, type AxiosResponse, type InternalAxiosRequestConfig } from 'axios'
import { afterEach, describe, expect, it } from 'vitest'
import { clearAccessToken, getAccessToken, setAccessToken } from '../token'
import { registerInterceptors } from './index'

function createRequest(adapter: AxiosAdapter) {
  const request = axios.create({ adapter })
  registerInterceptors(request)
  return request
}

function response(config: InternalAxiosRequestConfig, data: unknown, status = 200): AxiosResponse {
  return { config, data, headers: new AxiosHeaders(), status, statusText: 'OK' }
}

afterEach(() => clearAccessToken())

describe('request interceptors', () => {
  it('attaches the access token without replacing an explicit authorization header', async () => {
    setAccessToken('access-token')
    let receivedAuthorization: string | undefined
    const request = createRequest((config) => {
      const authorization = AxiosHeaders.from(config.headers).get('Authorization')
      receivedAuthorization = typeof authorization === 'string' ? authorization : undefined
      return Promise.resolve(response(config, { code: 200, data: true }))
    })

    await request.get('/profile')
    expect(receivedAuthorization).toBe('Bearer access-token')

    await request.get('/profile', { headers: { Authorization: 'Basic explicit' } })
    expect(receivedAuthorization).toBe('Basic explicit')
  })

  it('unwraps successful envelopes and rejects application errors', async () => {
    const request = createRequest((config) => Promise.resolve(response(config, { code: 200, data: { id: '1' } })))
    await expect(request.get('/projects')).resolves.toEqual({ id: '1' })

    const rejected = createRequest((config) => Promise.resolve(response(config, { code: 422, message: '字段无效', data: null })))
    await expect(rejected.get('/projects')).rejects.toMatchObject({
      name: 'ApiError',
      code: 422,
      status: 200,
      message: '字段无效',
    })
  })

  it('clears the token for unauthorized responses and normalizes transport failures', async () => {
    setAccessToken('access-token')
    const unauthorized = createRequest((config) => Promise.reject(new AxiosError('Unauthorized', undefined, config, undefined, response(config, { message: '未授权' }, 401))))
    await expect(unauthorized.get('/profile')).rejects.toMatchObject({ status: 401, message: '未授权' })
    expect(getAccessToken()).toBeNull()

    const timeout = createRequest((config) => Promise.reject(new AxiosError('Timeout', 'ECONNABORTED', config)))
    await expect(timeout.get('/projects')).rejects.toMatchObject({ message: '请求超时，请稍后重试' })

    const offline = createRequest((config) => Promise.reject(new AxiosError('Network Error', 'ERR_NETWORK', config)))
    await expect(offline.get('/projects')).rejects.toMatchObject({ message: '网络连接异常，请检查网络后重试' })
  })
})
