import { FileArrowDownIcon, ShieldCheckIcon, XIcon } from "@phosphor-icons/react";
import logoUrl from "../../assets/brand/xunqi-ui.png";
import { useI18n } from "../i18n";

type BrandAboutDialogProps = {
  onClose: () => void;
  onExportDiagnostics: () => void;
  diagnosticsBusy: boolean;
};

export function BrandAboutDialog({
  onClose,
  onExportDiagnostics,
  diagnosticsBusy,
}: BrandAboutDialogProps) {
  const { text } = useI18n();
  return (
    <div className="modal-backdrop" role="presentation">
      <section className="about-dialog" role="dialog" aria-modal="true" aria-labelledby="about-title">
        <header>
          <div className="about-brand">
            <img src={logoUrl} alt="" />
            <div>
              <span className="about-chapter">栖 · CHAPTER 01</span>
              <h2 id="about-title">讯栖 <span lang="en">XunQi</span></h2>
              <p>{text("讯来有迹，文止于栖。", "Messages traced, stories at rest.")}</p>
            </div>
          </div>
          <button type="button" onClick={onClose} aria-label={text("关闭关于讯栖", "Close About XunQi")}>
            <XIcon size={18} />
          </button>
        </header>
        <p className="about-intro">
          {text(
            "讯者，消息之所至；栖者，文章之所安。讯栖只接住你主动复制的微信分享链接，在本地巡视、整理并归栖，由你决定查看、导出或下载。",
            "XunQi receives only the WeChat share links you choose to copy. It inspects and organizes them locally, while you decide what to view, export, or download.",
          )}
        </p>
        <div className="about-principles">
          <article>
            <strong>{text("主动接收", "User-Initiated Capture")}</strong>
            <span>{text("不按名称订阅，不读取聊天记录，只处理你主动复制或粘贴的链接。", "No account-name subscriptions and no chat-history access. Only links you copy or paste are processed.")}</span>
          </article>
          <article>
            <strong>{text("本地归栖", "Local by Default")}</strong>
            <span>{text("任务、文章与导出结果优先留在你的电脑，不提供云端账号和遥测。", "Tasks, articles, and exports stay on your computer. XunQi has no cloud account or telemetry service.")}</span>
          </article>
          <article>
            <strong>{text("明确授权", "Explicit Authorization")}</strong>
            <span>{text("敏感辅助流程必须由你确认，结束后恢复网络并清理临时状态。", "Sensitive helper flows require your approval, then restore network settings and clear temporary state when they end.")}</span>
          </article>
        </div>
        <div className="about-diagnostics">
          <ShieldCheckIcon size={23} weight="duotone" />
          <div>
            <strong>{text("本地诊断日志", "Local Diagnostic Logs")}</strong>
            <span>
              {text(
                "只记录时间、功能事件、成功或失败分类、版本和系统类型；不记录 Cookie、聊天记录、文章正文、分享链接、密码或令牌。导出后可直接把 TXT 文件交给大师姐排查。",
                "Logs contain timestamps, feature events, result categories, version, and platform only. They exclude cookies, chat history, article text, share links, passwords, and tokens. Export the TXT file for troubleshooting.",
              )}
            </span>
          </div>
        </div>
        <footer>
          <span>A QIDU Utility</span>
          <div className="about-footer-actions">
            <button
              type="button"
              className="secondary-action"
              onClick={onExportDiagnostics}
              disabled={diagnosticsBusy}
            >
              <FileArrowDownIcon size={18} />
              {diagnosticsBusy ? text("正在导出…", "Exporting…") : text("导出诊断日志", "Export Diagnostic Log")}
            </button>
            <button type="button" className="primary-action" onClick={onClose}>{text("明白了", "Done")}</button>
          </div>
        </footer>
      </section>
    </div>
  );
}
