// A line of text, or a few lines, in the shape the pages already write a field: what an `<input>` takes, drawn by Astryx.
import type { ChangeEvent, CSSProperties, KeyboardEvent, FocusEvent, Ref } from 'react';
import { TextArea } from '@astryxdesign/core/TextArea';
import { TextInput } from '@astryxdesign/core/TextInput';

export interface FieldProps {
  id?: string;
  'aria-label'?: string;
  value?: string;
  onChange?: (event: ChangeEvent<HTMLInputElement>) => void;
  type?: 'text' | 'password' | 'search' | 'url' | 'email';
  placeholder?: string;
  maxLength?: number;
  disabled?: boolean;
  readOnly?: boolean;
  autoFocus?: boolean;
  /** More than one line. */
  rows?: number;
  /** Said by the old inputs; Astryx's field decides for itself. */
  spellCheck?: boolean;
  autoComplete?: string;
  autoCapitalize?: string;
  autoCorrect?: string;
  /** How wide the field is; without it, the width that the style sheet of the page gives it (`className`), or a default. */
  width?: number | string;
  className?: string;
  style?: CSSProperties;
  onFocus?: (event: FocusEvent<HTMLInputElement>) => void;
  onBlur?: (event: FocusEvent<HTMLInputElement>) => void;
  onKeyDown?: (event: KeyboardEvent<HTMLInputElement>) => void;
  ref?: Ref<HTMLInputElement>;
}

export function Field({ 'aria-label': label, onChange, rows, width, className, style, maxLength, autoFocus, readOnly, disabled, value = '', spellCheck: _s, autoComplete: _a, autoCapitalize: _c, autoCorrect: _o, ...rest }: FieldProps) {
  const sized = width ?? (className || style || rows ? undefined : 240);
  if (rows) {
    return (
      <TextArea
        label={label ?? ''}
        isLabelHidden
        rows={rows}
        width={sized ?? '100%'}
        htmlMaxLength={maxLength}
        isDisabled={disabled}
        isReadOnly={readOnly}
        hasSpellCheck={false}
        hasAutoFocus={autoFocus}
        containerClassName={className}
        containerStyle={style}
        value={value}
        placeholder={rest.placeholder}
        onFocus={rest.onFocus as never}
        onBlur={rest.onBlur as never}
        onChange={(_value, event) => onChange?.(event as unknown as ChangeEvent<HTMLInputElement>)}
      />
    );
  }
  return (
    <TextInput
      {...rest}
      value={value}
      label={label ?? ''}
      isLabelHidden
      size="sm"
      width={sized}
      isDisabled={disabled}
      isReadOnly={readOnly}
      hasAutoFocus={autoFocus}
      hasSpellCheck={false}
      autoComplete="off"
      autoCapitalize="off"
      containerClassName={className}
      containerStyle={style}
      onChange={(_value, event) => onChange?.(event)}
    />
  );
}
