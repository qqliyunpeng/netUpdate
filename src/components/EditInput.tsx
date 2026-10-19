import { type InputHTMLAttributes } from "react";
import { useEditContextMenu } from "./EditContextMenu";

type EditInputProps = Omit<InputHTMLAttributes<HTMLInputElement>, "onContextMenu">;

/** Text field with the same custom edit menu used by the noteCalc reference. */
export function EditInput(props: EditInputProps) {
  const { onContextMenu, menu } = useEditContextMenu();
  return (
    <>
      <input {...props} onContextMenu={onContextMenu} />
      {menu}
    </>
  );
}
