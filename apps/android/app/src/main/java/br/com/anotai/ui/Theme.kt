package br.com.anotai.ui

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color

/** Paleta da identidade AnotAI (mesma do desktop). */
object Brand {
    val Violet = Color(0xFF6D4AFF)
    val VioletLight = Color(0xFF9B85FF)
    val Coral = Color(0xFFFF5A5F)
    val Cyan = Color(0xFF18C8E0)
    val Night = Color(0xFF0E0B1F)
}

private val Light = lightColorScheme(
    primary = Brand.Violet,
    onPrimary = Color.White,
    secondary = Brand.Cyan,
    error = Brand.Coral,
    background = Color(0xFFFBFAFF),
    onBackground = Color(0xFF16132B),
    surface = Color.White,
    onSurface = Color(0xFF16132B),
    surfaceVariant = Color(0xFFF3F1FB),
    onSurfaceVariant = Color(0xFF615C7A),
    outline = Color(0xFFE4E0F2),
)

private val Dark = darkColorScheme(
    primary = Brand.VioletLight,
    onPrimary = Color.White,
    secondary = Brand.Cyan,
    error = Brand.Coral,
    background = Brand.Night,
    onBackground = Color(0xFFF1EFFA),
    surface = Color(0xFF15112A),
    onSurface = Color(0xFFF1EFFA),
    surfaceVariant = Color(0xFF1B1733),
    onSurfaceVariant = Color(0xFFA29DBD),
    outline = Color(0xFF2A2547),
)

@Composable
fun AnotAITheme(content: @Composable () -> Unit) {
    MaterialTheme(colorScheme = if (isSystemInDarkTheme()) Dark else Light, content = content)
}
