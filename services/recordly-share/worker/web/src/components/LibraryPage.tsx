import { Button, Card, Chip, Input, Label, Link, Skeleton, TextField, toast } from '@heroui/react';
import {
  ArrowClockwiseIcon,
  ArrowSquareOutIcon,
  LockKeyIcon,
  TrashIcon,
} from '@phosphor-icons/react';
import { useCallback, useEffect, useState } from 'react';
import {
  fetchVideos,
  renewVideo,
  deleteVideo,
  formatDate,
  formatDuration,
  daysUntilExpiry,
  type LibraryVideo,
} from '../scripts/library';
import { CenteredCard, CopyButton, Header, Notice, Notifications } from './ShareUI';

export function LibraryLogin() {
  const [error, setError] = useState(false);
  useEffect(() => setError(new URLSearchParams(location.search).has('error')), []);
  return (
    <CenteredCard>
      <LockKeyIcon size={28} />
      <h1>Your library</h1>
      <p>Enter your dashboard password to manage shared recordings.</p>
      <form method="POST" action="/library/login" className="gate-form">
        <TextField name="password" isRequired>
          <Label>Password</Label>
          <Input type="password" autoComplete="current-password" autoFocus />
        </TextField>
        {error && <Notice>Incorrect password. Please try again.</Notice>}
        <Button type="submit">Unlock library</Button>
      </form>
    </CenteredCard>
  );
}

export default function LibraryPage() {
  const [videos, setVideos] = useState<LibraryVideo[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');
  const [pending, setPending] = useState<string | null>(null);
  const [confirmDelete, setConfirmDelete] = useState<string | null>(null);
  const load = useCallback(async () => {
    setLoading(true);
    setError('');
    try {
      setVideos(await fetchVideos());
    } catch {
      setError('Could not load your recordings. Please try again.');
    } finally {
      setLoading(false);
    }
  }, []);
  useEffect(() => {
    void load();
  }, [load]);
  async function renew(code: string) {
    setPending(code);
    try {
      const expires = await renewVideo(code);
      if (!expires) throw new Error();
      setVideos((current) =>
        current.map((video) =>
          video.share_code === code ? { ...video, expires_at: expires } : video,
        ),
      );
      toast.success('Share link renewed');
    } catch {
      toast.danger('Could not renew this link. Try again.');
    } finally {
      setPending(null);
    }
  }
  async function remove(code: string) {
    setPending(code);
    try {
      if (!(await deleteVideo(code))) throw new Error();
      setVideos((current) => current.filter((video) => video.share_code !== code));
      toast.success('Recording deleted');
      setConfirmDelete(null);
    } catch {
      toast.danger('Could not delete this recording. Try again.');
    } finally {
      setPending(null);
    }
  }
  return (
    <>
      <Notifications />
      <Header title="Library" library />
      <main className="library-main">
        <div className="recording-heading">
          <div>
            <h1 className="recording-title">Your recordings</h1>
            <p className="recording-meta">Manage your shared videos and links.</p>
          </div>
          <Button variant="ghost" size="sm" onPress={() => void load()} isDisabled={loading}>
            <ArrowClockwiseIcon size={16} />
            Refresh
          </Button>
        </div>
        {error && <Notice>{error}</Notice>}
        <div className="library-grid">
          {loading
            ? [0, 1, 2].map((i) => <Skeleton key={i} className="loading-sidebar" />)
            : videos.map((video) => {
                const days = daysUntilExpiry(video.expires_at);
                return (
                  <Card key={video.share_code}>
                    <img
                      className="library-thumb"
                      src={`/thumb/${video.share_code}`}
                      alt=""
                      loading="lazy"
                      onError={(e) => {
                        e.currentTarget.style.visibility = 'hidden';
                      }}
                    />
                    <Card.Header>
                      <Card.Title>{video.title}</Card.Title>
                      <Card.Description>
                        {formatDuration(video.duration)} · {formatDate(video.created_at)} ·{' '}
                        {video.view_count} {video.view_count === 1 ? 'view' : 'views'}
                      </Card.Description>
                    </Card.Header>
                    <Card.Content>
                      <Chip size="sm" variant="soft" color={days <= 3 ? 'warning' : 'default'}>
                        {Number.isNaN(days)
                          ? 'Expiry unknown'
                          : days === 0
                            ? 'Expires today'
                            : days === 1
                              ? 'Expires tomorrow'
                              : `Expires in ${days} days`}
                      </Chip>
                      {!!video.is_protected && (
                        <Chip size="sm" variant="soft">
                          <LockKeyIcon size={12} />
                          Protected
                        </Chip>
                      )}
                    </Card.Content>
                    <Card.Footer className="library-actions">
                      <Link
                        className="button button--sm button--ghost"
                        href={`/s/${video.share_code}`}
                        target="_blank"
                        rel="noopener noreferrer"
                      >
                        <ArrowSquareOutIcon size={16} />
                        Open
                      </Link>
                      <CopyButton code={video.share_code} />
                      <Button
                        size="sm"
                        variant="ghost"
                        isDisabled={pending !== null}
                        onPress={() => void renew(video.share_code)}
                      >
                        Renew
                      </Button>
                      <Button
                        size="sm"
                        variant="danger-soft"
                        isIconOnly
                        aria-label={`Delete ${video.title}`}
                        isDisabled={pending !== null}
                        onPress={() => setConfirmDelete(video.share_code)}
                      >
                        <TrashIcon size={16} />
                      </Button>
                    </Card.Footer>
                    {confirmDelete === video.share_code && (
                      <Card.Content className="gate-content">
                        <Notice>
                          Delete “{video.title}”? This permanently removes the recording and its
                          share link.
                        </Notice>
                        <div className="library-actions">
                          <Button
                            size="sm"
                            variant="secondary"
                            onPress={() => setConfirmDelete(null)}
                            isDisabled={pending !== null}
                          >
                            Cancel
                          </Button>
                          <Button
                            size="sm"
                            variant="danger"
                            isPending={pending === video.share_code}
                            isDisabled={pending !== null}
                            onPress={() => void remove(video.share_code)}
                          >
                            Delete recording
                          </Button>
                        </div>
                      </Card.Content>
                    )}
                  </Card>
                );
              })}
        </div>
        {!loading && !error && videos.length === 0 && (
          <Card className="empty-feedback">
            <Card.Title>No shared recordings yet</Card.Title>
            <Card.Description>Publish a recording from Recordly to see it here.</Card.Description>
          </Card>
        )}
      </main>
    </>
  );
}
