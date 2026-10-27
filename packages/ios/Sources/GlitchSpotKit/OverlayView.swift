// ABOUTME: The SwiftUI overlay: drag to draw rectangles on the frozen frame, write notes, redact, then save or cancel.
#if os(iOS) && DEBUG
import SwiftUI

struct OverlayView: View {
    @Bindable var model: OverlayModel
    @State private var draft: CGRect?
    @State private var confirmingDiscard = false

    var body: some View {
        ZStack(alignment: .topLeading) {
            Image(uiImage: model.preview)
                .resizable()
                .frame(width: model.frame.size.width, height: model.frame.size.height)

            Color.clear
                .contentShape(Rectangle())
                .gesture(drawGesture)

            ForEach(Array(model.marks.enumerated()), id: \.element.id) { index, mark in
                MarkView(
                    mark: mark,
                    label: label(at: index),
                    note: noteBinding(mark.id),
                    onDelete: { model.delete(mark.id) }
                )
            }

            if let draft {
                Rectangle()
                    .stroke(Color(Palette.accent), style: StrokeStyle(lineWidth: 1.5, dash: [4, 3]))
                    .frame(width: draft.width, height: draft.height)
                    .offset(x: draft.minX, y: draft.minY)
                    .allowsHitTesting(false)
            }
        }
        .frame(width: model.frame.size.width, height: model.frame.size.height, alignment: .topLeading)
        .overlay(alignment: .top) { toolbar.padding(.top, 8) }
        .ignoresSafeArea(.container)
        .alert("Discard this snapshot?", isPresented: $confirmingDiscard) {
            Button("Discard", role: .destructive) { model.cancel() }
            Button("Keep editing", role: .cancel) {}
        }
    }

    private var drawGesture: some Gesture {
        DragGesture(minimumDistance: 4)
            .onChanged { value in
                draft = CGRect(origin: value.startLocation, size: .zero)
                    .union(CGRect(origin: value.location, size: .zero))
            }
            .onEnded { _ in
                defer { draft = nil }
                guard let rect = draft, rect.width >= 8, rect.height >= 8 else { return }
                model.marks.append(Mark(kind: model.tool == .redact ? .redaction : .annotation, rect: rect))
            }
    }

    private var toolbar: some View {
        HStack(spacing: 8) {
            Picker("Tool", selection: $model.tool) {
                Text("Annotate").tag(OverlayModel.Tool.annotate)
                Text("Redact").tag(OverlayModel.Tool.redact)
            }
            .pickerStyle(.segmented)
            .fixedSize()

            Button("Cancel") {
                if model.marks.isEmpty { model.cancel() } else { confirmingDiscard = true }
            }
            .buttonStyle(.plain)
            .foregroundStyle(Color(Palette.textPrimary))
            .padding(.horizontal, 10)
            .padding(.vertical, 6)
            .background(Color.white.opacity(0.07), in: RoundedRectangle(cornerRadius: 4))

            Button("Save") { Task { await model.save() } }
                .buttonStyle(.plain)
                .foregroundStyle(.white)
                .padding(.horizontal, 12)
                .padding(.vertical, 6)
                .background(Color(Palette.accent), in: RoundedRectangle(cornerRadius: 4))
                .disabled(model.marks.isEmpty || model.isSaving)
                .opacity(model.marks.isEmpty ? 0.5 : 1)
        }
        .font(.system(size: 14, weight: .medium))
        .padding(8)
        .background(Color(Palette.surfaceToolbar), in: Capsule())
        .overlay(Capsule().stroke(Color(Palette.borderSubtle), lineWidth: 1))
        .background(glitchSplit(Capsule()))
        .shadow(color: .black.opacity(0.6), radius: 10, y: 4)
        .environment(\.colorScheme, .dark)
    }

    /// Annotation labels count annotations only, so a redaction between two of them does not renumber them.
    private func label(at index: Int) -> String? {
        guard model.marks[index].kind == .annotation else { return nil }
        let number = model.marks[...index].filter { $0.kind == .annotation }.count
        return String(format: "%02d", number)
    }

    private func noteBinding(_ id: Mark.ID) -> Binding<String> {
        Binding(
            get: { model.marks.first { $0.id == id }?.note ?? "" },
            set: { text in
                if let i = model.marks.firstIndex(where: { $0.id == id }) { model.marks[i].note = text }
            }
        )
    }
}

private struct MarkView: View {
    let mark: Mark
    let label: String?
    @Binding var note: String
    let onDelete: () -> Void

    var body: some View {
        let rect = mark.rect
        ZStack(alignment: .topLeading) {
            if mark.kind == .annotation {
                Rectangle()
                    .fill(Color(Palette.accent).opacity(0.05))
                    .overlay(Rectangle().stroke(Color(Palette.accent).opacity(0.9), lineWidth: 1.5))
                    .background(glitchSplit(Rectangle()))
            } else {
                Rectangle()
                    .stroke(Color(Palette.textSecondary), style: StrokeStyle(lineWidth: 1, dash: [3, 3]))
            }
        }
        .frame(width: rect.width, height: rect.height)
        .overlay(alignment: .topLeading) {
            if let label {
                Text(label)
                    .font(.system(size: 11, weight: .bold))
                    .foregroundStyle(.white)
                    .frame(width: 26, height: 18)
                    .background(Color(Palette.accent), in: Capsule())
                    .offset(x: -13, y: -9)
            }
        }
        .overlay(alignment: .topTrailing) {
            Button(action: onDelete) {
                Image(systemName: "xmark")
                    .font(.system(size: 9, weight: .bold))
                    .foregroundStyle(.white)
                    .frame(width: 18, height: 18)
                    .background(Color(Palette.delete), in: Circle())
                    .shadow(color: .black.opacity(0.4), radius: 1.5, y: 1)
            }
            .accessibilityLabel("Delete rectangle")
            .offset(x: 9, y: -9)
        }
        .overlay(alignment: .topLeading) {
            if mark.kind == .annotation {
                NoteField(note: $note, tinted: mark.id.uuid.0 % 2 == 1)
                    .offset(y: rect.height + 6)
            }
        }
        .offset(x: rect.minX, y: rect.minY)
    }
}

private struct NoteField: View {
    @Binding var note: String
    let tinted: Bool

    var body: some View {
        TextField(
            "",
            text: $note,
            prompt: Text("Add a note").foregroundStyle(Color(Palette.accent).opacity(0.35)),
            axis: .vertical
        )
        .font(.system(size: 13))
        .lineSpacing(13 * 0.4)
        .foregroundStyle(Color(red: 170 / 255, green: 170 / 255, blue: 170 / 255))
        .padding(.horizontal, 8)
        .padding(.vertical, 6)
        .frame(width: 220, alignment: .leading)
        .background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: 5))
        .background(
            (tinted ? Color(red: 16 / 255, green: 22 / 255, blue: 24 / 255) : Color(red: 22 / 255, green: 22 / 255, blue: 22 / 255))
                .opacity(0.7),
            in: RoundedRectangle(cornerRadius: 5)
        )
        .overlay(RoundedRectangle(cornerRadius: 5).stroke(Color(Palette.accent).opacity(0.28), lineWidth: 1))
        .shadow(color: .black.opacity(0.5), radius: 5, y: 2)
        .environment(\.colorScheme, .dark)
    }
}

/// The DESIGN.md chromatic split: a red copy of the shape 2 pt left and a cyan copy 2 pt right, behind it.
private func glitchSplit<S: Shape>(_ shape: S) -> some View {
    ZStack {
        shape.stroke(Color(Palette.glitchRed), lineWidth: 2).offset(x: -2)
        shape.stroke(Color(Palette.glitchCyan), lineWidth: 2).offset(x: 2)
    }
    .allowsHitTesting(false)
}
#endif
