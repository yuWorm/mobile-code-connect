import { readFileSync } from 'node:fs'

import { describe, expect, test } from 'bun:test'

const browserView = new URL('../ServerAuthBrowserApproveView.vue', import.meta.url)
const deviceView = new URL('../ServerAuthDeviceApproveView.vue', import.meta.url)

describe('server auth approval views', () => {
  test('control api exposes browser approval for approval pages', () => {
    const source = readFileSync(new URL('../../lib/control/api.ts', import.meta.url), 'utf8')

    expect(source).toContain('approveBrowserServerAuth(sessionId: string)')
    expect(source).toContain("return this.get<BrowserServerAuthApprovalResponse>(`/server-auth/browser/approve?${params.toString()}`)")
    expect(source).toContain("new URLSearchParams({ session_id: sessionId })")
  })

  test('browser approval view loads detail and approves the session', () => {
    const source = readFileSync(browserView, 'utf8')

    expect(source).toContain("import { useRoute } from 'vue-router'")
    expect(source).toContain('const sessionId = String(route.query.session_id ?? \'\')')
    expect(source).toContain('controlApi.browserServerAuthSessionDetail(sessionId)')
    expect(source).toContain('controlApi.approveBrowserServerAuth(sessionId)')
    expect(source).toContain('approval.value?.server_auth_code')
    expect(source).toContain("copyToClipboard(approval.value.server_auth_code)")
  })

  test('device-code approval view accepts user codes from URL or input', () => {
    const source = readFileSync(deviceView, 'utf8')

    expect(source).toContain("import { useRoute } from 'vue-router'")
    expect(source).toContain("const userCodeInput = ref(String(route.query.user_code ?? ''))")
    expect(source).toContain('controlApi.deviceServerAuthSessionDetail(normalizedUserCode.value)')
    expect(source).toContain('controlApi.approveDeviceServerAuth(normalizedUserCode.value)')
    expect(source).toContain('controlApi.denyDeviceServerAuth(normalizedUserCode.value)')
    expect(source).toContain('<Input id="server-auth-user-code"')
  })

  test('approval views display generated device ids and public-key fingerprints', () => {
    for (const view of [browserView, deviceView]) {
      const source = readFileSync(view, 'utf8')

      expect(source).toContain('<InfoRow :label="t(\'common.deviceId\')" :value="detail.device_id" />')
      expect(source).toContain('<InfoRow :label="t(\'serverAuthApproval.publicKeyFingerprint\')" :value="detail.server_public_key_fingerprint" />')
      expect(source).toContain('<InfoRow :label="t(\'common.expiresAt\')" :value="formatEpoch(detail.expires_epoch_sec)" />')
    }
  })

  test('approval views keep visible copy behind i18n keys', () => {
    for (const view of [browserView, deviceView]) {
      const source = readFileSync(view, 'utf8')

      expect(source).toContain("import { useI18n } from '@/composables/useI18n'")
      expect(source).toContain('const { t } = useI18n()')
      expect(source).toContain("t('serverAuthApproval.")
      expect(source).not.toMatch(/>(?:Approve|Deny|Device Code|Server Login|Public Key|Copy|Retry)</)
      expect(source).not.toMatch(/>[^<{]*(?:审批|拒绝|设备码|公钥|复制|重试)[^<{]*</)
    }
  })
})
