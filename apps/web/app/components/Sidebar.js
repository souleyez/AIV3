'use client';

function DatasetCreateForm({
  draft,
  creating,
  onChange,
  onSubmit,
}) {
  return (
    <form
      className="side-card side-form"
      onSubmit={(event) => {
        event.preventDefault();
        onSubmit();
      }}
    >
      <div className="card-title">新建数据集</div>
      <label className="side-form-field">
        <span>Key</span>
        <input
          value={draft.key}
          onChange={(event) => onChange('key', event.target.value)}
          placeholder="例如 sales_briefing"
          disabled={creating}
        />
      </label>
      <label className="side-form-field">
        <span>标题</span>
        <input
          value={draft.title}
          onChange={(event) => onChange('title', event.target.value)}
          placeholder="例如 销售简报知识集"
          disabled={creating}
        />
      </label>
      <button className="primary-btn side-form-submit" type="submit" disabled={creating}>
        {creating ? '创建中...' : '创建并切换'}
      </button>
    </form>
  );
}

export default function Sidebar({
  datasets,
  selectedDatasetId,
  selectedDataset,
  datasetDraft,
  onDatasetDraftChange,
  onCreateDataset,
  onSelectDataset,
  creatingDataset,
  stats,
  loading,
  mobileOpen = false,
  onClose,
}) {
  return (
    <aside className={`sidebar ${mobileOpen ? 'open' : ''}`}>
      <div className="sidebar-mobile-head">
        <strong>数据集工作台</strong>
        <button type="button" className="ghost-btn compact-action-btn" onClick={onClose}>
          关闭
        </button>
      </div>

      <div className="brand">
        <div className="brand-logo">AI</div>
        <div>
          <h1>智能助手</h1>
          <p>V3 数据集工作台</p>
        </div>
      </div>

      <section className="nav-section">
        <div className="nav-title">当前能力</div>
        <div className="nav-item nav-item-static active">数据集切换</div>
        <div className="nav-item nav-item-static">会话回看</div>
        <div className="nav-item nav-item-static">资料输出 / 发布结果</div>
      </section>

      <section className="side-card">
        <div className="card-title">数据集</div>
        <div className="dataset-list">
          {datasets.length ? (
            datasets.map((dataset) => {
              const active = dataset.id === selectedDatasetId;
              return (
                <button
                  key={dataset.id}
                  type="button"
                  className={`dataset-item ${active ? 'active' : ''}`}
                  onClick={() => onSelectDataset(dataset.id)}
                  disabled={loading && !active}
                >
                  <span className="dataset-item-title">{dataset.title}</span>
                  <span className="dataset-item-meta">
                    {dataset.key} · {dataset.lifecycle}
                  </span>
                </button>
              );
            })
          ) : (
            <div className="side-card-empty">
              还没有数据集。先在下面创建一个，前端就能切进对应工作台。
            </div>
          )}
        </div>
      </section>

      <DatasetCreateForm
        draft={datasetDraft}
        creating={creatingDataset}
        onChange={onDatasetDraftChange}
        onSubmit={onCreateDataset}
      />

      <section className="side-card compact">
        <div className="card-title">当前上下文</div>
        <div className="side-kv-list">
          <div className="side-kv-row">
            <span>数据集</span>
            <strong>{selectedDataset ? selectedDataset.title : '未选择'}</strong>
          </div>
          <div className="side-kv-row">
            <span>会话</span>
            <strong>{stats.sessions}</strong>
          </div>
          <div className="side-kv-row">
            <span>资料输出</span>
            <strong>{stats.outputs}</strong>
          </div>
          <div className="side-kv-row">
            <span>报告计划</span>
            <strong>{stats.plans}</strong>
          </div>
          <div className="side-kv-row">
            <span>已发布</span>
            <strong>{stats.published}</strong>
          </div>
        </div>
      </section>

      <section className="side-card compact">
        <div className="card-title">当前约束</div>
        <p>未选会话时发送问题会新建 `chat_session workflow`；选中历史会话后会追加新一轮，右侧保留资料输出和发布结果回看。</p>
      </section>
    </aside>
  );
}
