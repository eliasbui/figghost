import { Component } from 'react';
import type { ReactNode } from 'react';
import { send } from '@/stores/app';

interface Props {
  /** The page it holds: a new page starts over. */
  page: string;
  /** What is said in place of a page that could not be drawn. */
  message: string;
  children: ReactNode;
}

/** A page that fails to draw leaves the rest of the window as it was, says so, and tells the app what went wrong (which
 *  writes it in the log), instead of taking the whole window with it. */
export class PageBoundary extends Component<Props, { failed: boolean }> {
  state = { failed: false };

  static getDerivedStateFromError() {
    return { failed: true };
  }

  componentDidCatch(error: Error) {
    send({ t: 'js_error', message: `page ${this.props.page}: ${error.message}`, line: 0 });
  }

  componentDidUpdate(prev: Props) {
    if (prev.page !== this.props.page && this.state.failed) this.setState({ failed: false });
  }

  render() {
    if (!this.state.failed) return this.props.children;
    return (
      <section className="card" role="alert">
        <div className="row">
          <div className="row-text">
            <div className="row-title">
              <span className="row-name">{this.props.message}</span>
            </div>
          </div>
        </div>
      </section>
    );
  }
}
