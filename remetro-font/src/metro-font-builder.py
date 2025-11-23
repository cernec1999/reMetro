#!/usr/bin/env python3
"""
WMATA Metro Font Builder

- Load a photo
- Drag 4 corner handles to outline the display
- Perspective-correct to (192 x 64) * scale (default scale = 6)
- Overlay 192 x 64 grid on preview
- Export corrected PNG of grid cell centers
- Build a dynamic font by sampling letter regions

Dependencies: pip install pyqt5 pillow numpy
"""

import json
import math
import os
import statistics
import sys
from dataclasses import dataclass
from typing import Dict, List, Optional, Tuple, cast

import numpy as np
from pcffont import PcfFontBuilder, PcfGlyph
from PIL import Image, ImageDraw
from PyQt5 import QtCore, QtGui, QtWidgets

from u8g2 import export_u8g2_from_font_data

OUTPUT_COLS = 192
# In reality, this is 64, but we have additional padding on top and bottom
# and it's hard to get exact cropping since they're not visible in the photo
OUTPUT_ROWS = 62
SCALE = 6
OUT_W = OUTPUT_COLS * SCALE
OUT_H = OUTPUT_ROWS * SCALE
DEFAULT_LETTER_COLS = 11
DEFAULT_LETTER_ROWS = 14
ASCII_FIRST = 32
ASCII_LAST = 126
ASCII_LEN = ASCII_LAST - ASCII_FIRST + 1  # 95


@dataclass
class Point:
    """A simple 2D point with x and y coordinates."""

    x: float
    y: float


def find_perspective_transform(
    src_pts: List[Point], dst_pts: List[Point]
) -> np.ndarray:
    """Find the perspective transformation matrix from source to destination points.

    Args:
        src_pts: List of 4 source points
        dst_pts: List of 4 destination points

    Returns:
        3x3 transformation matrix as numpy array

    Raises:
        ValueError: If not exactly 4 points provided for src and dst
    """
    if len(src_pts) != 4 or len(dst_pts) != 4:
        raise ValueError("Need 4 src and 4 dst points")
    A = []
    for sp, dp in zip(src_pts, dst_pts):
        x, y = sp.x, sp.y
        u, v = dp.x, dp.y
        A.append([x, y, 1, 0, 0, 0, -u * x, -u * y, -u])
        A.append([0, 0, 0, x, y, 1, -v * x, -v * y, -v])
    A = np.array(A, dtype=np.float64)
    _, _, Vt = np.linalg.svd(A)
    h = Vt[-1, :]
    H = h.reshape((3, 3))
    return H


def warp_perspective_pillow(
    image: Image.Image, H: np.ndarray, dst_size: Tuple[int, int]
) -> Image.Image:
    """Apply perspective transformation to an image using PIL.

    Args:
        image: Source PIL Image
        H: 3x3 transformation matrix
        dst_size: Output image size as (width, height)

    Returns:
        Transformed PIL Image
    """
    w, h = dst_size
    H_inv = np.linalg.inv(H)
    if H_inv[2, 2] != 0:
        H_inv = H_inv / H_inv[2, 2]
    coeffs = (
        H_inv[0, 0],
        H_inv[0, 1],
        H_inv[0, 2],
        H_inv[1, 0],
        H_inv[1, 1],
        H_inv[1, 2],
        H_inv[2, 0],
        H_inv[2, 1],
    )
    return image.transform((w, h), Image.PERSPECTIVE, coeffs, resample=Image.BICUBIC)


def pil_to_qpixmap(pil_img: Image.Image) -> QtGui.QPixmap:
    """Convert a PIL Image to a Qt QPixmap.

    Args:
        pil_img: PIL Image to convert

    Returns:
        QPixmap representation of the image
    """
    if pil_img.mode != "RGBA":
        img = pil_img.convert("RGBA")
    else:
        img = pil_img
    w, h = img.size
    data = img.tobytes("raw", "RGBA")
    qimg = QtGui.QImage(data, w, h, 4 * w, QtGui.QImage.Format_RGBA8888)
    return QtGui.QPixmap.fromImage(qimg)


class ZoomableGraphicsView(QtWidgets.QGraphicsView):
    """A QGraphicsView with zoom and pan capabilities using mouse and gestures."""

    def __init__(self, parent=None):
        """Initialize the zoomable graphics view."""
        super().__init__(parent)
        self._zoom_level: float = 0.0
        self._zoom_step = 1.25
        self._zoom_range = (-10, 30)
        self._in_fit = False
        self.setTransformationAnchor(QtWidgets.QGraphicsView.AnchorUnderMouse)
        self.setResizeAnchor(QtWidgets.QGraphicsView.AnchorUnderMouse)
        self.setDragMode(QtWidgets.QGraphicsView.NoDrag)
        self.setViewportUpdateMode(QtWidgets.QGraphicsView.SmartViewportUpdate)
        self.viewport().setAttribute(QtCore.Qt.WA_AcceptTouchEvents, True)
        self.grabGesture(QtCore.Qt.PinchGesture)
        self._pinch_initial_zoom = 0.0
        self._pinch_active = False
        self._pan_key_active = False

    def wheelEvent(self, event: QtGui.QWheelEvent):
        """Handle mouse wheel events for zooming when Ctrl is held."""
        if self._pinch_active:
            event.accept()
            return
        if event.modifiers() & QtCore.Qt.ControlModifier:
            delta = event.angleDelta().y()
            if delta == 0:
                event.ignore()
                return
            factor = self._zoom_step if delta > 0 else 1 / self._zoom_step
            level_delta = 1 if delta > 0 else -1
            self._apply_zoom_factor(factor, level_delta)
            event.accept()
            return
        super().wheelEvent(event)

    def fit_view(self, rect):
        """Fit the view to show the specified rectangle."""
        if rect.isNull():
            return
        self._in_fit = True
        try:
            self._zoom_level = 0
            self.setTransform(QtGui.QTransform())
            self.fitInView(rect, QtCore.Qt.KeepAspectRatio)
        finally:
            self._in_fit = False

    def fit_to_scene(self):
        """Fit the view to show the entire scene."""
        scene = self.scene()
        if scene is None:
            return
        rect = scene.sceneRect()
        if rect.isNull():
            return
        self._in_fit = True
        try:
            self._zoom_level = 0
            self.setTransform(QtGui.QTransform())
            self.fitInView(rect, QtCore.Qt.KeepAspectRatio)
        finally:
            self._in_fit = False

    def resizeEvent(self, event: QtGui.QResizeEvent):
        """Handle widget resize events."""
        super().resizeEvent(event)
        if self._zoom_level == 0 and not self._in_fit:
            self.fit_to_scene()

    def event(self, event: QtCore.QEvent):
        if event.type() == QtCore.QEvent.Gesture:
            # Cast to QGestureEvent to access gesture() method
            gesture_event = cast(QtWidgets.QGestureEvent, event)
            gesture = gesture_event.gesture(QtCore.Qt.PinchGesture)
            if gesture is not None:
                # Cast to QPinchGesture for proper typing
                pinch_gesture = cast(QtWidgets.QPinchGesture, gesture)
                self._handle_pinch(pinch_gesture)
                return True
        return super().event(event)

    def keyPressEvent(self, event: QtGui.QKeyEvent):
        if event.key() == QtCore.Qt.Key_Space and not event.isAutoRepeat():
            self._set_pan_active(True)
            event.accept()
            return
        super().keyPressEvent(event)

    def keyReleaseEvent(self, event: QtGui.QKeyEvent):
        if event.key() == QtCore.Qt.Key_Space and not event.isAutoRepeat():
            self._set_pan_active(False)
            event.accept()
            return
        super().keyReleaseEvent(event)

    def mousePressEvent(self, event: QtGui.QMouseEvent):
        if self._pan_key_active and event.button() == QtCore.Qt.LeftButton:
            self.viewport().setCursor(QtCore.Qt.ClosedHandCursor)
        super().mousePressEvent(event)

    def mouseReleaseEvent(self, event: QtGui.QMouseEvent):
        super().mouseReleaseEvent(event)
        if event.button() == QtCore.Qt.LeftButton:
            if self._pan_key_active:
                self.viewport().setCursor(QtCore.Qt.OpenHandCursor)
            else:
                self.viewport().unsetCursor()

    def is_pan_active(self) -> bool:
        return self._pan_key_active

    def _set_pan_active(self, active: bool):
        if self._pan_key_active == active:
            return
        self._pan_key_active = active
        if active:
            self.setDragMode(QtWidgets.QGraphicsView.ScrollHandDrag)
            self.viewport().setCursor(QtCore.Qt.OpenHandCursor)
        else:
            self.setDragMode(QtWidgets.QGraphicsView.NoDrag)
            self.viewport().unsetCursor()

    def _apply_zoom_factor(self, factor: float, level_delta_hint: float = None):
        if factor == 1.0:
            return
        if level_delta_hint is None:
            level_delta = math.log(factor, self._zoom_step)
        else:
            level_delta = level_delta_hint
        self._set_zoom_level(self._zoom_level + level_delta)

    def _set_zoom_level(self, target: float):
        clamped = max(self._zoom_range[0], min(target, self._zoom_range[1]))
        delta = clamped - self._zoom_level
        if abs(delta) < 1e-6:
            return
        factor = self._zoom_step**delta
        self.scale(factor, factor)
        self._zoom_level = clamped

    def _handle_pinch(self, gesture: QtWidgets.QPinchGesture):
        state = gesture.state()
        if state == QtCore.Qt.GestureStarted:
            self._pinch_active = True
            self._pinch_initial_zoom = self._zoom_level
            return
        if state in (
            QtCore.Qt.GestureUpdated,
            QtCore.Qt.GestureFinished,
            QtCore.Qt.GestureCanceled,
        ):
            total_scale = gesture.totalScaleFactor()
            if total_scale <= 0:
                return
            level_delta = math.log(total_scale, self._zoom_step)
            self._set_zoom_level(self._pinch_initial_zoom + level_delta)
            if state in (QtCore.Qt.GestureFinished, QtCore.Qt.GestureCanceled):
                self._pinch_active = False


class CellItem(QtWidgets.QGraphicsRectItem):
    """A clickable cell in the grid editor representing a single pixel."""

    def __init__(self, row: int, col: int, size: float):
        """Initialize a cell item.

        Args:
            row: Grid row position
            col: Grid column position
            size: Size of the cell square in pixels
        """
        super().__init__(0, 0, size, size)
        self.row = row
        self.col = col
        self._on_brush = QtGui.QBrush(QtGui.QColor(255, 140, 0, 220))
        self._off_brush = QtGui.QBrush(QtGui.QColor(0, 0, 0, 0))
        self._pen = QtGui.QPen(QtGui.QColor(0, 200, 120, 140))
        self._pen.setWidthF(0)
        self.setPen(self._pen)
        self.setBrush(self._off_brush)
        self.setZValue(2)
        self.setAcceptedMouseButtons(QtCore.Qt.NoButton)
        self.state = False

    def set_state(self, state: bool):
        self.state = state
        self.setBrush(self._on_brush if state else self._off_brush)

    def update_position(self, size: float, row_offset: int, col_offset: int):
        self.setRect(0, 0, size, size)
        self.setPos((col_offset + self.col) * size, (row_offset + self.row) * size)

    def set_grid_visible(self, visible: bool):
        color = QtGui.QColor(0, 200, 120, 140 if visible else 0)
        self._pen.setColor(color)
        self.setPen(self._pen)


class GridEditorWidget(ZoomableGraphicsView):
    """Interactive grid editor for editing letter bitmaps with zoom and pan support."""

    offsetsChanged = QtCore.pyqtSignal(int, int)
    letterSizeChanged = QtCore.pyqtSignal(int, int)

    def __init__(self, parent=None):
        """Initialize the grid editor widget."""
        super().__init__(parent)
        self.setRenderHints(
            QtGui.QPainter.Antialiasing | QtGui.QPainter.SmoothPixmapTransform
        )
        self.setFocusPolicy(QtCore.Qt.StrongFocus)
        self._scene = QtWidgets.QGraphicsScene(self)
        self.setScene(self._scene)
        self._scene.setSceneRect(0, 0, OUT_W, OUT_H)

        self.pixmap_item = QtWidgets.QGraphicsPixmapItem()
        self.pixmap_item.setZValue(0)
        self._scene.addItem(self.pixmap_item)

        self.selection_rect = QtWidgets.QGraphicsRectItem()
        sel_pen = QtGui.QPen(QtGui.QColor(255, 215, 0, 220))
        sel_pen.setWidth(2)
        sel_pen.setStyle(QtCore.Qt.DashLine)
        self.selection_rect.setPen(sel_pen)
        self.selection_rect.setBrush(QtGui.QBrush(QtGui.QColor(255, 215, 0, 60)))
        self.selection_rect.setZValue(1)
        self._scene.addItem(self.selection_rect)

        self.row_offset = 0
        self.col_offset = 0
        self.letter_rows = DEFAULT_LETTER_ROWS
        self.letter_cols = DEFAULT_LETTER_COLS
        self.bitmap = np.zeros((self.letter_rows, self.letter_cols), dtype=bool)
        self.cells: List[List[CellItem]] = []
        self._create_cells()
        self._update_selection_rect()
        self._current_image: Optional[Image.Image] = None
        self._dragging_selection = False
        self._drag_start_scene = QtCore.QPointF()
        self._initial_row_offset = 0
        self._initial_col_offset = 0
        self._stroke_active = False
        self._stroke_mode = True
        self.fit_to_scene()

    def _create_cells(self):
        size = SCALE
        self.cells = []
        for r in range(self.letter_rows):
            row_cells = []
            for c in range(self.letter_cols):
                cell = CellItem(r, c, size)
                cell.update_position(size, self.row_offset, self.col_offset)
                self._scene.addItem(cell)
                row_cells.append(cell)
            self.cells.append(row_cells)

    def _update_cell_positions(self):
        size = SCALE
        for row in self.cells:
            for cell in row:
                cell.update_position(size, self.row_offset, self.col_offset)

    def _update_selection_rect(self):
        size = SCALE
        self.selection_rect.setRect(
            self.col_offset * size,
            self.row_offset * size,
            self.letter_cols * size,
            self.letter_rows * size,
        )

    def _refresh_cells_from_bitmap(self):
        size = SCALE
        for r in range(self.letter_rows):
            row_cells = self.cells[r]
            for c in range(self.letter_cols):
                cell = row_cells[c]
                cell.row = r
                cell.col = c
                cell.update_position(size, self.row_offset, self.col_offset)
                cell.set_state(bool(self.bitmap[r, c]))

    def _set_cell_state(self, row: int, col: int, state: bool):
        state = bool(state)
        if self.bitmap[row, col] == state and self.cells[row][col].state == state:
            return
        self.cells[row][col].set_state(state)
        self.bitmap[row, col] = state

    def set_letter_width(self, cols: int):
        """Set the letter width in columns, updating the grid layout.

        Args:
            cols: Number of columns (will be clamped to valid range)
        """
        cols = max(1, min(cols, OUTPUT_COLS))
        if cols == self.letter_cols:
            return

        old_cols = self.letter_cols
        self.letter_cols = cols

        new_bitmap = np.zeros((self.letter_rows, self.letter_cols), dtype=bool)
        min_cols = min(old_cols, self.letter_cols)
        new_bitmap[:, :min_cols] = self.bitmap[:, :min_cols]
        self.bitmap = new_bitmap

        for r in range(self.letter_rows):
            row_cells = self.cells[r]
            if self.letter_cols > old_cols:
                for c in range(old_cols, self.letter_cols):
                    cell = CellItem(r, c, SCALE)
                    self._scene.addItem(cell)
                    row_cells.append(cell)
            elif self.letter_cols < old_cols:
                for _ in range(old_cols - 1, self.letter_cols - 1, -1):
                    cell = row_cells.pop()
                    self._scene.removeItem(cell)

        max_col_offset = max(0, OUTPUT_COLS - self.letter_cols)
        self.col_offset = min(self.col_offset, max_col_offset)

        self._update_selection_rect()
        self._refresh_cells_from_bitmap()

        self.offsetsChanged.emit(self.row_offset, self.col_offset)
        self.letterSizeChanged.emit(self.letter_rows, self.letter_cols)

    def _cell_from_scene_pos(
        self, scene_pos: QtCore.QPointF
    ) -> Optional[Tuple[int, int]]:
        size = SCALE
        if scene_pos.x() < 0 or scene_pos.y() < 0:
            return None
        col = int(scene_pos.x() // size) - self.col_offset
        row = int(scene_pos.y() // size) - self.row_offset
        if 0 <= row < self.letter_rows and 0 <= col < self.letter_cols:
            return row, col
        return None

    def _begin_stroke(self, row: int, col: int):
        self._stroke_active = True
        self._stroke_mode = not self.cells[row][col].state
        self._apply_stroke(row, col)

    def _apply_stroke(self, row: int, col: int):
        target_state = bool(self._stroke_mode)
        self._set_cell_state(row, col, target_state)

    def _apply_stroke_from_scene(self, scene_pos: QtCore.QPointF):
        coords = self._cell_from_scene_pos(scene_pos)
        if coords is None:
            return
        row, col = coords
        self._apply_stroke(row, col)

    def _end_stroke(self):
        self._stroke_active = False

    def set_image(self, pil_img: Image.Image):
        """Set the background image for the grid editor.

        Args:
            pil_img: PIL Image to display as background
        """
        self._current_image = pil_img.copy()
        self.pixmap_item.setPixmap(pil_to_qpixmap(pil_img))
        self._scene.setSceneRect(0, 0, pil_img.width, pil_img.height)
        self.fit_to_scene()

    def clear_bitmap(self):
        """Clear all cells in the bitmap, setting them to False/empty."""
        for r in range(self.letter_rows):
            for c in range(self.letter_cols):
                self._set_cell_state(r, c, False)

    def set_bitmap(self, data):
        """Set the bitmap data for the current letter.

        Args:
            data: 2D array-like of boolean values representing the bitmap

        Raises:
            ValueError: If bitmap shape doesn't match current letter dimensions
        """
        if data is None:
            return
        arr = np.array(data, dtype=bool)
        if arr.shape != (self.letter_rows, self.letter_cols):
            raise ValueError("Bitmap shape mismatch")
        for r in range(self.letter_rows):
            for c in range(self.letter_cols):
                self._set_cell_state(r, c, bool(arr[r, c]))

    def get_bitmap(self):
        """Get the current bitmap as a list of lists of integers (0/1).

        Returns:
            2D list of integers representing the bitmap
        """
        return self.bitmap.astype(int).tolist()

    def populate_from_image(
        self, pil_img: Optional[Image.Image] = None, threshold: int = 128
    ):
        source = pil_img or self._current_image
        if source is None:
            return
        gray = source.convert("L")
        arr = np.array(gray)
        for r in range(self.letter_rows):
            for c in range(self.letter_cols):
                src_r = self.row_offset + r
                src_c = self.col_offset + c
                row_start = src_r * SCALE
                row_end = min(row_start + SCALE, arr.shape[0])
                col_start = src_c * SCALE
                col_end = min(col_start + SCALE, arr.shape[1])
                patch = arr[row_start:row_end, col_start:col_end]
                value = bool(patch.size and float(patch.mean()) < threshold)
                self._set_cell_state(r, c, value)

    def set_show_grid(self, show: bool):
        for row in self.cells:
            for cell in row:
                cell.set_grid_visible(show)
        pen = self.selection_rect.pen()
        if show:
            pen.setColor(QtGui.QColor(255, 215, 0, 220))
            self.selection_rect.setBrush(QtGui.QBrush(QtGui.QColor(255, 215, 0, 60)))
            self.selection_rect.setVisible(True)
        else:
            pen.setColor(QtGui.QColor(255, 215, 0, 0))
            self.selection_rect.setBrush(QtGui.QBrush(QtGui.QColor(0, 0, 0, 0)))
            self.selection_rect.setVisible(False)
        self.selection_rect.setPen(pen)

    def set_offsets(self, row: int, col: int):
        row = max(0, min(row, OUTPUT_ROWS - self.letter_rows))
        col = max(0, min(col, OUTPUT_COLS - self.letter_cols))
        if row == self.row_offset and col == self.col_offset:
            return
        self.row_offset = row
        self.col_offset = col
        self._update_cell_positions()
        self._update_selection_rect()
        self.offsetsChanged.emit(self.row_offset, self.col_offset)

    def get_offsets(self) -> Tuple[int, int]:
        return self.row_offset, self.col_offset

    def mousePressEvent(self, event: QtGui.QMouseEvent):
        modifiers = event.modifiers()

        if self.is_pan_active():
            super().mousePressEvent(event)
            return

        if event.button() == QtCore.Qt.LeftButton:
            if modifiers & QtCore.Qt.MetaModifier:
                scene_pos = self.mapToScene(event.pos())
                rect_scene = self.selection_rect.mapRectToScene(
                    self.selection_rect.rect()
                )
                if rect_scene.contains(scene_pos):
                    self._dragging_selection = True
                    self._drag_start_scene = scene_pos
                    self._initial_row_offset = self.row_offset
                    self._initial_col_offset = self.col_offset
                    event.accept()
                    return
            elif not modifiers & (QtCore.Qt.MetaModifier | QtCore.Qt.ControlModifier):
                scene_pos = self.mapToScene(event.pos())
                coords = self._cell_from_scene_pos(scene_pos)
                if coords is not None:
                    row, col = coords
                    self._begin_stroke(row, col)
                    event.accept()
                    return

        super().mousePressEvent(event)

    def mouseMoveEvent(self, event: QtGui.QMouseEvent):
        if self._dragging_selection:
            scene_pos = self.mapToScene(event.pos())
            delta = scene_pos - self._drag_start_scene
            col_delta = int(round(delta.x() / SCALE))
            row_delta = int(round(delta.y() / SCALE))
            new_row = self._initial_row_offset + row_delta
            new_col = self._initial_col_offset + col_delta
            self.set_offsets(new_row, new_col)
            event.accept()
            return
        if self._stroke_active and (event.buttons() & QtCore.Qt.LeftButton):
            scene_pos = self.mapToScene(event.pos())
            self._apply_stroke_from_scene(scene_pos)
            event.accept()
            return
        super().mouseMoveEvent(event)

    def mouseReleaseEvent(self, event: QtGui.QMouseEvent):
        if self._dragging_selection and event.button() == QtCore.Qt.LeftButton:
            self._dragging_selection = False
            event.accept()
            return
        if self._stroke_active and event.button() == QtCore.Qt.LeftButton:
            self._end_stroke()
            event.accept()
            return
        super().mouseReleaseEvent(event)

    def keyPressEvent(self, event: QtGui.QKeyEvent):
        modifiers = event.modifiers()
        disallowed = modifiers & ~(QtCore.Qt.ShiftModifier | QtCore.Qt.KeypadModifier)
        if disallowed:
            event.accept()
            return

        key = event.key()
        if key in (
            QtCore.Qt.Key_Left,
            QtCore.Qt.Key_Right,
            QtCore.Qt.Key_Up,
            QtCore.Qt.Key_Down,
        ):
            horizontal_step = (
                1 if (modifiers & QtCore.Qt.ShiftModifier) else self.letter_cols + 1
            )
            vertical_step = (
                1 if (modifiers & QtCore.Qt.ShiftModifier) else self.letter_rows + 2
            )

            new_row = self.row_offset
            new_col = self.col_offset

            if key == QtCore.Qt.Key_Left:
                new_col -= horizontal_step
            elif key == QtCore.Qt.Key_Right:
                new_col += horizontal_step
            elif key == QtCore.Qt.Key_Up:
                new_row -= vertical_step
            elif key == QtCore.Qt.Key_Down:
                new_row += vertical_step

            self.set_offsets(new_row, new_col)
            event.accept()
            return

        if key == QtCore.Qt.Key_BracketLeft:
            step = 1 if not (modifiers & QtCore.Qt.ShiftModifier) else 2
            self.set_letter_width(self.letter_cols - step)
            event.accept()
            return

        if key == QtCore.Qt.Key_BracketRight:
            step = 1 if not (modifiers & QtCore.Qt.ShiftModifier) else 2
            self.set_letter_width(self.letter_cols + step)
            event.accept()
            return

        super().keyPressEvent(event)


class LetterPreviewCard(QtWidgets.QFrame):
    """A preview card showing a letter and its bitmap representation."""

    def __init__(self, letter: str, bitmap):
        """Initialize the letter preview card.

        Args:
            letter: The character this card represents
            bitmap: 2D array of the letter's bitmap data
        """
        super().__init__()
        self.setFrameShape(QtWidgets.QFrame.StyledPanel)
        layout = QtWidgets.QVBoxLayout(self)
        layout.setContentsMargins(8, 8, 8, 8)
        layout.setSpacing(6)
        title = QtWidgets.QLabel(letter.upper())
        title.setAlignment(QtCore.Qt.AlignCenter)
        title.setStyleSheet("font-weight: bold;")
        layout.addWidget(title)
        preview = QtWidgets.QLabel()
        preview.setAlignment(QtCore.Qt.AlignCenter)
        preview.setPixmap(render_letter_pixmap(bitmap))
        layout.addWidget(preview)


def render_letter_pixmap(bitmap, scale: int = 12) -> QtGui.QPixmap:
    arr = np.array(bitmap, dtype=np.uint8)
    if arr.size == 0:
        arr = np.zeros((DEFAULT_LETTER_ROWS, DEFAULT_LETTER_COLS), dtype=np.uint8)
    rows, cols = arr.shape
    image = QtGui.QImage(cols * scale, rows * scale, QtGui.QImage.Format_ARGB32)
    image.fill(QtGui.QColor(15, 15, 15, 255))
    painter = QtGui.QPainter(image)
    pen_grid = QtGui.QPen(QtGui.QColor(70, 70, 70))
    pen_grid.setWidth(1)
    brush_on = QtGui.QBrush(QtGui.QColor(255, 140, 0))
    for r in range(rows):
        for c in range(cols):
            rect = QtCore.QRect(c * scale, r * scale, scale, scale)
            if arr[r, c]:
                painter.fillRect(rect, brush_on)
            painter.setPen(pen_grid)
            painter.drawRect(rect)
    painter.end()
    return QtGui.QPixmap.fromImage(image)


class LetterPreviewWidget(QtWidgets.QScrollArea):
    """Scrollable widget for displaying letter preview cards."""

    def __init__(self, parent=None):
        super().__init__(parent)
        self.setWidgetResizable(True)
        self._container = QtWidgets.QWidget()
        self.setWidget(self._container)
        self._layout = QtWidgets.QGridLayout(self._container)
        self._layout.setAlignment(QtCore.Qt.AlignTop | QtCore.Qt.AlignLeft)
        self._layout.setHorizontalSpacing(12)
        self._layout.setVerticalSpacing(12)

    def _clear(self):
        while self._layout.count():
            item = self._layout.takeAt(0)
            widget = item.widget()
            if widget is not None:
                widget.deleteLater()

    def update_letters(self, font_data: Dict[str, List[List[int]]]):
        self._clear()
        if not font_data:
            placeholder = QtWidgets.QLabel("No letters saved yet.")
            placeholder.setAlignment(QtCore.Qt.AlignCenter)
            self._layout.addWidget(placeholder, 0, 0)
            self._layout.setAlignment(placeholder, QtCore.Qt.AlignCenter)
            return
        columns = 8
        for idx, (letter, bitmap) in enumerate(sorted(font_data.items())):
            card = LetterPreviewCard(letter, bitmap)
            row = idx // columns
            col = idx % columns
            self._layout.addWidget(card, row, col)


class HandleItem(QtWidgets.QGraphicsObject):
    """Draggable handle for image correction."""

    moved = QtCore.pyqtSignal(float, float, int)

    def __init__(self, idx, x, y, radius=6, parent=None):
        super().__init__(parent)
        self.radius = radius
        self._brush = QtGui.QBrush(QtGui.QColor("#00ffff"))
        self._pen = QtGui.QPen(QtCore.Qt.black, 1)
        self.setFlag(QtWidgets.QGraphicsItem.ItemIsMovable, True)
        self.setFlag(QtWidgets.QGraphicsItem.ItemSendsGeometryChanges, True)
        self.setZValue(2)
        self.idx = idx
        self.setPos(x, y)

    def boundingRect(self) -> QtCore.QRectF:
        r = self.radius
        return QtCore.QRectF(-r, -r, 2 * r, 2 * r)

    def paint(self, painter: QtGui.QPainter, option, widget=None):
        # pylint: disable=unused-argument  # Qt API requirement
        painter.setPen(self._pen)
        painter.setBrush(self._brush)
        painter.drawEllipse(self.boundingRect())

    def itemChange(self, change, value):
        if change == QtWidgets.QGraphicsItem.ItemPositionChange:
            new_pos = value
            self.moved.emit(new_pos.x(), new_pos.y(), self.idx)
        return super().itemChange(change, value)


class QuadScene(QtWidgets.QGraphicsScene):
    """Graphics scene for quadrilateral image correction."""

    def __init__(self, pixmap: QtGui.QPixmap, image_size: Tuple[int, int]):
        super().__init__()
        self.img_item = self.addPixmap(pixmap)
        self.img_item.setZValue(0)
        self.poly_item = self.addPolygon(
            QtGui.QPolygonF(),
            QtGui.QPen(QtGui.QColor("#00ffff"), 2, QtCore.Qt.DashLine),
        )
        self.poly_item.setZValue(1)
        w, h = image_size
        margin = 40
        self.points = [
            QtCore.QPointF(margin, margin),
            QtCore.QPointF(w - margin, margin),
            QtCore.QPointF(w - margin, h - margin),
            QtCore.QPointF(margin, h - margin),
        ]
        self.handles: List[HandleItem] = []
        for i, p in enumerate(self.points):
            handle = HandleItem(i, p.x(), p.y())
            handle.moved.connect(self.on_handle_moved)
            self.addItem(handle)
            self.handles.append(handle)
        self.update_polygon()

    def on_handle_moved(self, x: float, y: float, idx: int):
        self.points[idx] = QtCore.QPointF(x, y)
        self.update_polygon()

    def update_polygon(self):
        self.poly_item.setPolygon(QtGui.QPolygonF(self.points))

    def get_points(self) -> List[Point]:
        return [Point(p.x(), p.y()) for p in self.points]


class PreviewWidget(QtWidgets.QLabel):
    """Widget for displaying image previews."""

    def __init__(self):
        super().__init__()
        self.setAlignment(QtCore.Qt.AlignCenter)
        self.setMinimumSize(OUT_W // 3, OUT_H // 3)
        self._pix: Optional[QtGui.QPixmap] = None
        self.show_grid = True
        self._letter_outline: Optional[Tuple[int, int, int, int]] = None

    def set_letter_outline(self, row: int, col: int, rows: int, cols: int):
        self._letter_outline = (row, col, rows, cols)

    def set_image(self, pil_img: Image.Image):
        img = pil_img.copy()
        draw = ImageDraw.Draw(img)
        if self.show_grid:
            step_x = OUT_W / OUTPUT_COLS
            step_y = OUT_H / OUTPUT_ROWS
            for c in range(OUTPUT_COLS + 1):
                x = int(round(c * step_x))
                color = (0, 200, 120) if c % 8 == 0 else (0, 120, 90)
                draw.line([(x, 0), (x, OUT_H)], fill=color, width=1)
            for r in range(OUTPUT_ROWS + 1):
                y = int(round(r * step_y))
                color = (0, 200, 120) if r % 8 == 0 else (0, 120, 90)
                draw.line([(0, y), (OUT_W, y)], fill=color, width=1)
        if self._letter_outline is not None:
            row, col, rows, cols = self._letter_outline
            x0 = col * SCALE
            y0 = row * SCALE
            x1 = x0 + cols * SCALE
            y1 = y0 + rows * SCALE
            draw.rectangle((x0, y0, x1, y1), outline=(255, 215, 0), width=2)
        pm = pil_to_qpixmap(img.resize((OUT_W // 3, OUT_H // 3), Image.NEAREST))
        self._pix = pm
        self.setPixmap(pm)


class MainWindow(QtWidgets.QMainWindow):
    """Main application window for the WMATA Metro Font Builder.

    Provides tools for perspective correction of metro display photos
    and building fonts from the corrected images.
    """

    def __init__(self):
        """Initialize the main window with all UI components."""
        super().__init__()
        self.setWindowTitle("WMATA Dot Matrix Helper (PyQt5)")
        self.resize(1200, 800)

        self.image: Optional[Image.Image] = None
        self.image_path: Optional[str] = None
        self.corrected: Optional[Image.Image] = None

        # Initialize all attributes that will be set in helper methods
        self.graphics_view: Optional[ZoomableGraphicsView] = None
        self.grid_editor: Optional[GridEditorWidget] = None
        self.central_tabs: Optional[QtWidgets.QTabWidget] = None
        self.letter_preview: Optional[LetterPreviewWidget] = None
        self.letter_preview_index: int = 0
        self.lbl_aspect: Optional[QtWidgets.QLabel] = None
        self.chk_grid: Optional[QtWidgets.QCheckBox] = None
        self.threshold_spin: Optional[QtWidgets.QSpinBox] = None
        self.row_offset_spin: Optional[QtWidgets.QSpinBox] = None
        self.col_offset_spin: Optional[QtWidgets.QSpinBox] = None
        self.preview: Optional[PreviewWidget] = None
        self.letter_input: Optional[QtWidgets.QLineEdit] = None
        self.letters_list: Optional[QtWidgets.QListWidget] = None
        self.scene: Optional[QuadScene] = None
        self.font_data: Dict[str, List[List[int]]] = {}

        self._setup_central_widgets()
        self._setup_sidebar()
        self._finalize_layout()

    def _setup_central_widgets(self):
        """Set up the main central widget area with tabs."""
        self.graphics_view = ZoomableGraphicsView()
        self.graphics_view.setRenderHint(QtGui.QPainter.Antialiasing, True)

        self.grid_editor = GridEditorWidget()
        self.grid_editor.setEnabled(False)
        self.grid_editor.offsetsChanged.connect(self.on_grid_offsets_changed)
        self.grid_editor.letterSizeChanged.connect(self.on_letter_size_changed)

        self.central_tabs = QtWidgets.QTabWidget()
        self.central_tabs.addTab(self.graphics_view, "Image")
        self.central_tabs.addTab(self.grid_editor, "Grid Editor")
        self.central_tabs.setTabEnabled(1, False)
        self.letter_preview = LetterPreviewWidget()
        self.letter_preview_index = self.central_tabs.addTab(
            self.letter_preview, "Letter Preview"
        )
        self.central_tabs.setTabEnabled(self.letter_preview_index, False)
        self.setCentralWidget(self.central_tabs)

    def _setup_sidebar(self):
        """Set up the sidebar with all controls."""
        side = QtWidgets.QWidget()
        layout = QtWidgets.QVBoxLayout(side)

        self._setup_image_controls(layout)
        self._setup_grid_controls(layout)
        self._setup_font_controls(layout)

        dock = QtWidgets.QDockWidget("Tools", self)
        dock.setWidget(side)
        self.addDockWidget(QtCore.Qt.RightDockWidgetArea, dock)

    def _setup_image_controls(self, layout: QtWidgets.QVBoxLayout):
        """Set up image loading and perspective correction controls."""
        btn_open = QtWidgets.QPushButton("Open Image…")
        btn_open.clicked.connect(self.open_image)
        layout.addWidget(btn_open)

        self.lbl_aspect = QtWidgets.QLabel(
            f"Output: {OUTPUT_COLS}x{OUTPUT_ROWS} (scaled x{SCALE})"
        )
        layout.addWidget(self.lbl_aspect)

        btn_apply = QtWidgets.QPushButton("Apply Perspective")
        btn_apply.clicked.connect(self.apply_perspective)
        layout.addWidget(btn_apply)

        self.chk_grid = QtWidgets.QCheckBox("Show Grid")
        self.chk_grid.setChecked(True)
        self.chk_grid.stateChanged.connect(self.update_preview)
        layout.addWidget(self.chk_grid)

    def _setup_grid_controls(self, layout: QtWidgets.QVBoxLayout):
        """Set up grid editing and preview controls."""
        thresh_layout = QtWidgets.QHBoxLayout()
        thresh_layout.addWidget(QtWidgets.QLabel("Auto threshold"))
        self.threshold_spin = QtWidgets.QSpinBox()
        self.threshold_spin.setRange(0, 255)
        self.threshold_spin.setValue(128)
        thresh_layout.addWidget(self.threshold_spin)
        layout.addLayout(thresh_layout)

        self._setup_offset_controls(layout)

        btn_auto_fill = QtWidgets.QPushButton("Auto-fill from image")
        btn_auto_fill.clicked.connect(self.auto_fill_grid)
        layout.addWidget(btn_auto_fill)

        btn_export = QtWidgets.QPushButton("Export as PNG")
        btn_export.clicked.connect(self.export_data)
        layout.addWidget(btn_export)

        self.preview = PreviewWidget()
        layout.addWidget(self.preview, 1)

    def _setup_offset_controls(self, layout: QtWidgets.QVBoxLayout):
        """Set up the letter position offset controls."""
        offset_layout = QtWidgets.QHBoxLayout()
        offset_layout.addWidget(QtWidgets.QLabel("Letter origin"))
        offset_layout.addSpacing(6)
        offset_layout.addWidget(QtWidgets.QLabel("Row"))
        self.row_offset_spin = QtWidgets.QSpinBox()
        self.row_offset_spin.setRange(0, max(0, OUTPUT_ROWS - DEFAULT_LETTER_ROWS))
        self.row_offset_spin.setValue(0)
        offset_layout.addWidget(self.row_offset_spin)
        offset_layout.addSpacing(6)
        offset_layout.addWidget(QtWidgets.QLabel("Col"))
        self.col_offset_spin = QtWidgets.QSpinBox()
        self.col_offset_spin.setRange(0, max(0, OUTPUT_COLS - DEFAULT_LETTER_COLS))
        self.col_offset_spin.setValue(0)
        offset_layout.addWidget(self.col_offset_spin)
        layout.addLayout(offset_layout)
        self.row_offset_spin.valueChanged.connect(self.on_offsets_changed)
        self.col_offset_spin.valueChanged.connect(self.on_offsets_changed)

    def _setup_font_controls(self, layout: QtWidgets.QVBoxLayout):
        """Set up font building and export controls."""
        font_header = QtWidgets.QLabel("Font builder")
        font_header.setStyleSheet("font-weight: bold;")
        layout.addWidget(font_header)

        self._setup_letter_input(layout)
        self._setup_letter_buttons(layout)

        self.letters_list = QtWidgets.QListWidget()
        self.letters_list.itemSelectionChanged.connect(self.load_selected_letter)
        layout.addWidget(self.letters_list, 1)

        self._setup_font_actions(layout)
        self._setup_export_buttons(layout)

    def _setup_letter_input(self, layout: QtWidgets.QVBoxLayout):
        """Set up letter input controls."""
        letter_row = QtWidgets.QHBoxLayout()
        self.letter_input = QtWidgets.QLineEdit()
        self.letter_input.setPlaceholderText("Letter (e.g. A)")
        self.letter_input.setMaxLength(2)
        letter_row.addWidget(self.letter_input)
        btn_save_letter = QtWidgets.QPushButton("Save letter")
        btn_save_letter.clicked.connect(self.save_current_letter)
        letter_row.addWidget(btn_save_letter)
        layout.addLayout(letter_row)

    def _setup_letter_buttons(self, layout: QtWidgets.QVBoxLayout):
        """Set up letter manipulation buttons."""
        btns_row = QtWidgets.QHBoxLayout()
        btn_clear = QtWidgets.QPushButton("Clear grid")
        btn_clear.clicked.connect(self.clear_grid_cells)
        btn_remove = QtWidgets.QPushButton("Remove letter")
        btn_remove.clicked.connect(self.remove_selected_letter)
        btns_row.addWidget(btn_clear)
        btns_row.addWidget(btn_remove)
        layout.addLayout(btns_row)

    def _setup_font_actions(self, layout: QtWidgets.QVBoxLayout):
        """Set up font load/save buttons."""
        font_actions = QtWidgets.QHBoxLayout()
        btn_load_font = QtWidgets.QPushButton("Load font…")
        btn_load_font.clicked.connect(self.load_font_file)
        btn_save_font = QtWidgets.QPushButton("Save font…")
        btn_save_font.clicked.connect(self.save_font_file)
        font_actions.addWidget(btn_load_font)
        font_actions.addWidget(btn_save_font)
        layout.addLayout(font_actions)

    def _setup_export_buttons(self, layout: QtWidgets.QVBoxLayout):
        """Set up font export buttons."""
        btn_export_pcf = QtWidgets.QPushButton("Export PCF font…")
        btn_export_pcf.clicked.connect(self.export_pcf_font)
        layout.addWidget(btn_export_pcf)

        btn_export_mtr = QtWidgets.QPushButton("Export U8G2 font…")
        btn_export_mtr.clicked.connect(self.export_u8g2_font_v1)
        layout.addWidget(btn_export_mtr)

    def _finalize_layout(self):
        """Finalize the layout and initialize state."""
        self.scene = None
        self.grid_editor.set_offsets(0, 0)
        self.preview.set_letter_outline(
            0, 0, self.grid_editor.letter_rows, self.grid_editor.letter_cols
        )
        self.update_letter_preview_tab()

    def on_offsets_changed(self):
        row = self.row_offset_spin.value()
        col = self.col_offset_spin.value()
        self.grid_editor.set_offsets(row, col)

    def on_grid_offsets_changed(self, row: int, col: int):
        with QtCore.QSignalBlocker(self.row_offset_spin):
            self.row_offset_spin.setValue(row)
        with QtCore.QSignalBlocker(self.col_offset_spin):
            self.col_offset_spin.setValue(col)
        self.preview.set_letter_outline(
            row, col, self.grid_editor.letter_rows, self.grid_editor.letter_cols
        )
        self.update_preview()

    def on_letter_size_changed(self, rows: int, cols: int):
        max_row_offset = max(0, OUTPUT_ROWS - rows)
        max_col_offset = max(0, OUTPUT_COLS - cols)

        current_row, current_col = self.grid_editor.get_offsets()
        clamped_row = min(current_row, max_row_offset)
        clamped_col = min(current_col, max_col_offset)
        if (clamped_row, clamped_col) != (current_row, current_col):
            self.grid_editor.set_offsets(clamped_row, clamped_col)
            current_row, current_col = clamped_row, clamped_col

        with QtCore.QSignalBlocker(self.row_offset_spin):
            self.row_offset_spin.setRange(0, max_row_offset)
            self.row_offset_spin.setValue(current_row)

        with QtCore.QSignalBlocker(self.col_offset_spin):
            self.col_offset_spin.setRange(0, max_col_offset)
            self.col_offset_spin.setValue(current_col)

        self.preview.set_letter_outline(current_row, current_col, rows, cols)
        self.update_preview()

    def update_letter_preview_tab(self):
        self.letter_preview.update_letters(self.font_data)
        has_letters = bool(self.font_data)
        self.central_tabs.setTabEnabled(self.letter_preview_index, has_letters)
        if (
            not has_letters
            and self.central_tabs.currentIndex() == self.letter_preview_index
        ):
            if self.grid_editor.isEnabled():
                self.central_tabs.setCurrentWidget(self.grid_editor)
            else:
                self.central_tabs.setCurrentWidget(self.graphics_view)

    def open_image(self):
        path, _ = QtWidgets.QFileDialog.getOpenFileName(
            self,
            "Open image",
            "",
            "Images (*.png *.jpg *.jpeg *.bmp *.tif *.tiff);;All Files (*)",
        )
        if not path:
            return
        self.image_path = path
        self.image = Image.open(path).convert("RGB")
        pix = QtGui.QPixmap(path)
        if pix.isNull():
            QtWidgets.QMessageBox.critical(
                self, "Error", "Failed to load image into QPixmap."
            )
            return
        self.scene = QuadScene(pix, (pix.width(), pix.height()))
        self.graphics_view.setScene(self.scene)
        self.graphics_view.fit_to_scene()
        self.preview.setText("(no preview yet)")
        self.corrected = None
        self.grid_editor.clear_bitmap()
        self.grid_editor.setEnabled(False)
        self.central_tabs.setTabEnabled(1, False)
        self.central_tabs.setCurrentWidget(self.graphics_view)

    def apply_perspective(self):
        if self.image is None or self.scene is None:
            return
        src_pts = self.scene.get_points()
        dst_pts = [
            Point(0, 0),
            Point(OUT_W - 1, 0),
            Point(OUT_W - 1, OUT_H - 1),
            Point(0, OUT_H - 1),
        ]
        H = find_perspective_transform(src_pts, dst_pts)
        self.corrected = warp_perspective_pillow(self.image, H, (OUT_W, OUT_H))
        self.grid_editor.set_image(self.corrected)
        self.grid_editor.set_offsets(
            self.row_offset_spin.value(), self.col_offset_spin.value()
        )
        self.grid_editor.populate_from_image(
            self.corrected, self.threshold_spin.value()
        )
        self.grid_editor.setEnabled(True)
        self.central_tabs.setTabEnabled(1, True)
        self.central_tabs.setCurrentWidget(self.grid_editor)
        self.grid_editor.setFocus(QtCore.Qt.OtherFocusReason)
        self.update_preview()

    def update_preview(self):
        if self.corrected is None:
            return
        row, col = self.grid_editor.get_offsets()
        self.preview.set_letter_outline(
            row, col, self.grid_editor.letter_rows, self.grid_editor.letter_cols
        )
        self.preview.show_grid = self.chk_grid.isChecked()
        self.preview.set_image(self.corrected)
        self.grid_editor.set_show_grid(self.chk_grid.isChecked())

    def auto_fill_grid(self):
        if self.corrected is None or not self.grid_editor.isEnabled():
            return
        self.grid_editor.populate_from_image(
            self.corrected, self.threshold_spin.value()
        )

    def save_current_letter(self):
        if not self.grid_editor.isEnabled():
            QtWidgets.QMessageBox.information(self, "Font", "Apply perspective first.")
            return
        letter = self.letter_input.text()
        if letter == "":
            QtWidgets.QMessageBox.information(
                self, "Font", "Enter a letter name first."
            )
            return
        # Allow space character by checking for explicit " " input
        if letter == " ":
            letter = " "
        else:
            letter = letter.strip()
            if not letter:
                QtWidgets.QMessageBox.information(
                    self, "Font", "Enter a letter name first."
                )
                return
            letter = letter[0]
        self.font_data[letter] = self.grid_editor.get_bitmap()
        self.refresh_letters_list(select_letter=letter)

    def clear_grid_cells(self):
        self.grid_editor.clear_bitmap()

    def remove_selected_letter(self):
        items = self.letters_list.selectedItems()
        if not items:
            return
        letter = items[0].text()
        if letter in self.font_data:
            del self.font_data[letter]
        self.refresh_letters_list()

    def load_selected_letter(self):
        items = self.letters_list.selectedItems()
        if not items:
            return
        letter = items[0].text()
        data = self.font_data.get(letter)
        if data is None:
            return
        if not isinstance(data, list) or not data:
            QtWidgets.QMessageBox.warning(
                self, "Font", "Stored letter data is empty or invalid."
            )
            return
        if any(not isinstance(row, list) for row in data):
            QtWidgets.QMessageBox.warning(
                self, "Font", "Stored letter has invalid row data."
            )
            return
        row_lengths = {len(row) for row in data}
        if not row_lengths or len(row_lengths) != 1:
            QtWidgets.QMessageBox.warning(
                self, "Font", "Stored letter has inconsistent row lengths."
            )
            return
        cols = row_lengths.pop()
        if cols <= 0 or cols > OUTPUT_COLS:
            QtWidgets.QMessageBox.warning(
                self, "Font", "Stored letter has an invalid width."
            )
            return
        if len(data) != self.grid_editor.letter_rows:
            message = (
                f"Stored letter height {len(data)} does not match "
                f"editor height {self.grid_editor.letter_rows}."
            )
            QtWidgets.QMessageBox.warning(self, "Font", message)
            return
        self.grid_editor.set_letter_width(cols)
        try:
            self.grid_editor.set_bitmap(data)
        except ValueError as exc:
            QtWidgets.QMessageBox.warning(
                self, "Font", f"Failed to load letter data: {exc}"
            )

    def save_font_file(self):
        if not self.font_data:
            QtWidgets.QMessageBox.information(self, "Font", "No letters to save yet.")
            return
        path, _ = QtWidgets.QFileDialog.getSaveFileName(
            self, "Save font", "font.json", "JSON (*.json)"
        )
        if not path:
            return
        with open(path, "w", encoding="utf-8") as fh:
            json.dump(self.font_data, fh, indent=2)

    def load_font_file(self):
        path, _ = QtWidgets.QFileDialog.getOpenFileName(
            self, "Load font", "", "JSON (*.json)"
        )
        if not path:
            return
        try:
            with open(path, "r", encoding="utf-8") as fh:
                data = json.load(fh)
        except (OSError, json.JSONDecodeError) as exc:
            QtWidgets.QMessageBox.warning(
                self, "Font", f"Could not load font file: {exc}"
            )
            return
        cleaned: Dict[str, List[List[int]]] = {}
        for letter, bitmap in data.items():
            if not letter or not isinstance(bitmap, list):
                continue
            if not bitmap:
                continue
            if any(not isinstance(row, list) for row in bitmap):
                continue
            row_lengths = {len(row) for row in bitmap}
            if len(row_lengths) != 1:
                continue
            rows = len(bitmap)
            cols = row_lengths.pop()
            if rows != DEFAULT_LETTER_ROWS or cols <= 0 or cols > OUTPUT_COLS:
                continue
            try:
                arr = np.array(bitmap, dtype=int)
            except ValueError:
                continue
            cleaned[letter[0]] = (arr > 0).astype(int).tolist()
        self.font_data = cleaned
        self.refresh_letters_list()

    def export_u8g2_font_v1(self):
        path, _ = QtWidgets.QFileDialog.getSaveFileName(
            self, "Export U8g2 font", "metro_u8g2.bin", "U8g2 Font (*.bin)"
        )
        if not path:
            return
        export_u8g2_from_font_data(self.font_data, path)

    def export_pcf_font(self):
        """Export the current font data as a PCF font file."""
        if not self.font_data:
            QtWidgets.QMessageBox.information(
                self, "PCF Export", "No letters to export yet."
            )
            return

        path, _ = QtWidgets.QFileDialog.getSaveFileName(
            self, "Export PCF font", "metro_font.pcf", "PCF Font (*.pcf)"
        )
        if not path:
            return

        try:
            # Create PCF font builder
            builder = PcfFontBuilder()

            # Configure font metrics - assuming Metro display font characteristics
            builder.config.font_ascent = DEFAULT_LETTER_ROWS
            builder.config.font_descent = 0

            EXTRA_TRACK = 1  # We add 1 pixel of extra tracking
            BASELINE_OFFSET = 0  # Baseline offset from bottom

            # Add glyphs for each letter
            for letter, bitmap in self.font_data.items():
                bitmap_array = np.array(bitmap, dtype=int)
                rows, cols = bitmap_array.shape

                lsb = 0  # left side bearing in pixels (set >0 if you want space before)
                advance = lsb + cols + EXTRA_TRACK

                glyph = PcfGlyph(
                    name=letter,
                    encoding=ord(letter),
                    scalable_width=advance
                    * 1000,  # SWIDTH in 1/1000 em (rough but consistent)
                    character_width=advance,  # the actual advance width in pixels
                    dimensions=(cols, rows),  # bitmap bounding box
                    offset=(lsb, BASELINE_OFFSET),  # (left bearing, vertical offset)
                    bitmap=bitmap_array.tolist(),
                )
                builder.glyphs.append(glyph)

            # Set font properties
            builder.properties.foundry = "reMetro"
            builder.properties.family_name = "Metro Display"
            builder.properties.weight_name = "Medium"
            builder.properties.slant = "R"  # Roman (upright)
            builder.properties.setwidth_name = "Normal"
            builder.properties.add_style_name = "Dot Matrix"
            builder.properties.pixel_size = DEFAULT_LETTER_ROWS
            builder.properties.point_size = builder.properties.pixel_size * 10
            builder.properties.resolution_x = 75
            builder.properties.resolution_y = 75
            builder.properties.spacing = "P"  # Proportional

            # Calculate average width
            if builder.glyphs:
                builder.properties.average_width = round(
                    statistics.fmean(
                        glyph.character_width * 10 for glyph in builder.glyphs
                    )
                )
            else:
                builder.properties.average_width = DEFAULT_LETTER_COLS * 10

            builder.properties.charset_registry = "ISO10646"
            builder.properties.charset_encoding = "1"
            builder.properties.generate_xlfd()

            # Additional properties
            builder.properties.x_height = DEFAULT_LETTER_ROWS // 2
            builder.properties.cap_height = DEFAULT_LETTER_ROWS
            builder.properties.underline_position = 0
            builder.properties.underline_thickness = 0
            builder.properties.font_version = "1.0.0"
            builder.properties.copyright = "GPLv3"

            # Save the PCF font
            builder.save(path)

            QtWidgets.QMessageBox.information(
                self,
                "PCF Export",
                f"Successfully exported PCF font to {path}\n\n"
                f"Font contains {len(builder.glyphs)} characters.",
            )

        except (OSError, ValueError, RuntimeError) as e:
            QtWidgets.QMessageBox.critical(
                self, "PCF Export Error", f"Failed to export PCF font:\n{str(e)}"
            )

    def refresh_letters_list(self, select_letter: Optional[str] = None):
        with QtCore.QSignalBlocker(self.letters_list):
            self.letters_list.clear()
            for letter in sorted(self.font_data.keys()):
                self.letters_list.addItem(letter)
        if select_letter:
            matches = self.letters_list.findItems(select_letter, QtCore.Qt.MatchExactly)
            if matches:
                self.letters_list.setCurrentItem(matches[0])
        self.update_letter_preview_tab()

    def export_data(self):
        if self.corrected is None:
            QtWidgets.QMessageBox.information(
                self, "Export", "Apply perspective first."
            )
            return
        outdir = QtWidgets.QFileDialog.getExistingDirectory(
            self, "Choose export folder"
        )
        if not outdir:
            return
        base = os.path.splitext(os.path.basename(self.image_path or "output"))[0]
        img_path = os.path.join(outdir, f"{base}_corrected_{OUT_W}x{OUT_H}.png")
        self.corrected.save(img_path)


def main():
    """Main entry point for the application."""
    app = QtWidgets.QApplication(sys.argv)
    window = MainWindow()
    window.show()
    sys.exit(app.exec_())


if __name__ == "__main__":
    main()
