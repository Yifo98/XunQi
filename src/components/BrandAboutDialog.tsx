import { XIcon } from "@phosphor-icons/react";
import logoUrl from "../../assets/brand/xunqi-ui.png";

type BrandAboutDialogProps = {
  onClose: () => void;
};

export function BrandAboutDialog({ onClose }: BrandAboutDialogProps) {
  return (
    <div className="modal-backdrop" role="presentation">
      <section className="about-dialog" role="dialog" aria-modal="true" aria-labelledby="about-title">
        <header>
          <div className="about-brand">
            <img src={logoUrl} alt="" />
            <div>
              <span className="about-chapter">栖 · CHAPTER 01</span>
              <h2 id="about-title">讯栖 <span lang="en">XunQi</span></h2>
              <p>讯来有迹，文止于栖。</p>
            </div>
          </div>
          <button type="button" onClick={onClose} aria-label="关闭关于讯栖">
            <XIcon size={18} />
          </button>
        </header>
        <p className="about-intro">
          讯者，消息之所至；栖者，文章之所安。讯栖只接住你主动复制的微信分享链接，
          在本地巡视、整理并归栖，由你决定查看、导出或下载。
        </p>
        <div className="about-principles">
          <article>
            <strong>主动接收</strong>
            <span>不按名称订阅，不读取聊天记录，只处理你主动复制或粘贴的链接。</span>
          </article>
          <article>
            <strong>本地归栖</strong>
            <span>任务、文章与导出结果优先留在你的电脑，不提供云端账号和遥测。</span>
          </article>
          <article>
            <strong>明确授权</strong>
            <span>敏感辅助流程必须由你确认，结束后恢复网络并清理临时状态。</span>
          </article>
        </div>
        <footer>
          <span>A QIDU Utility</span>
          <button type="button" className="primary-action" onClick={onClose}>明白了</button>
        </footer>
      </section>
    </div>
  );
}
