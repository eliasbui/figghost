import { InfoRow, Card } from '@/components/macos/Layout';
import { useApp } from '@/stores/app';

/** The page of a feature that has no folder under pages/ yet. */
export default function Pending() {
  const page = useApp((s) => s.page);
  return (
    <Card>
      <InfoRow title={page} desc="This page has not been moved to the new window yet." />
    </Card>
  );
}
