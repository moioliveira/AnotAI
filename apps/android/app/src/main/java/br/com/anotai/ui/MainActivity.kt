package br.com.anotai.ui

import android.Manifest
import android.content.Intent
import android.content.pm.ApplicationInfo
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.provider.Settings
import androidx.activity.ComponentActivity
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.scale
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import br.com.anotai.AnotAIApp
import br.com.anotai.R
import br.com.anotai.assistant.Assistant
import br.com.anotai.service.WakeService
import java.text.DateFormat
import java.util.Date

class MainActivity : ComponentActivity() {
    private val assistant get() = (application as AnotAIApp).assistant

    /** Muda a cada volta ao app: a tela relê permissões e notas. */
    private val resumeTick = mutableIntStateOf(0)

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        setContent {
            AnotAITheme {
                Surface(Modifier.fillMaxSize(), color = MaterialTheme.colorScheme.background) {
                    AnotAIScreen(assistant, resumeTick.intValue)
                }
            }
        }
        handle(intent)
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        handle(intent)
    }

    override fun onResume() {
        super.onResume()
        resumeTick.intValue++
    }

    private fun handle(intent: Intent?) {
        if (intent?.action != ACTION_LISTEN) return
        // Só em build de debug: comando em texto vindo do adb, para testes
        // automáticos. Em release, nenhum outro app pode disparar ações aqui.
        val debuggable = applicationInfo.flags and ApplicationInfo.FLAG_DEBUGGABLE != 0
        val typed = intent.getStringExtra(EXTRA_TEXT)
        if (debuggable && !typed.isNullOrBlank()) {
            assistant.handleTyped(typed)
            return
        }
        if (checkSelfPermission(Manifest.permission.RECORD_AUDIO) == PackageManager.PERMISSION_GRANTED) {
            assistant.listen()
        }
    }

    companion object {
        const val ACTION_LISTEN = "br.com.anotai.LISTEN"
        private const val EXTRA_TEXT = "text"
    }
}

private data class Perm(val name: String, val label: String)

private val PERMISSIONS = buildList {
    add(Perm(Manifest.permission.RECORD_AUDIO, "Microfone"))
    add(Perm(Manifest.permission.READ_CONTACTS, "Contatos"))
    add(Perm(Manifest.permission.READ_CALENDAR, "Agenda"))
    add(Perm(Manifest.permission.SEND_SMS, "SMS"))
    add(Perm(Manifest.permission.CALL_PHONE, "Ligações"))
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
        add(Perm(Manifest.permission.POST_NOTIFICATIONS, "Notificações"))
    }
}

private val EXAMPLES = listOf(
    "Bom dia AnotAI, me acorde às 6 e meia",
    "Boa tarde AnotAI, mande uma mensagem para a Maria",
    "Boa noite AnotAI, qual a minha agenda de amanhã?",
    "Oi AnotAI, marque reunião com o João sexta às 10",
    "Oi AnotAI, me lembre de tomar o remédio às 8 da noite",
    "Oi AnotAI, ligue para a mãe",
    "Oi AnotAI, anota aí comprar pão",
    "Oi AnotAI, timer de 10 minutos",
    "Oi AnotAI, acenda a lanterna",
)

@Composable
private fun AnotAIScreen(assistant: Assistant, resumeTick: Int) {
    val context = LocalContext.current
    val ui by assistant.ui.collectAsState()
    var permTick by remember { mutableIntStateOf(0) }
    val missing = remember(resumeTick, permTick, ui.missingPermission) {
        PERMISSIONS.filter {
            context.checkSelfPermission(it.name) != PackageManager.PERMISSION_GRANTED
        }
    }
    val requestPerms = rememberLauncherForActivityResult(
        ActivityResultContracts.RequestMultiplePermissions(),
    ) {
        permTick++
        assistant.clearMissingPermission()
    }
    val micGranted = missing.none { it.name == Manifest.permission.RECORD_AUDIO }
    val overlayGranted = remember(resumeTick) { Settings.canDrawOverlays(context) }

    Column(
        Modifier
            .fillMaxSize()
            .windowInsetsPadding(WindowInsets.safeDrawing)
            .verticalScroll(rememberScrollState())
            .padding(horizontal = 20.dp, vertical = 16.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Column {
            Text("AnotAI", style = MaterialTheme.typography.headlineMedium, fontWeight = FontWeight.Bold)
            Text(
                "Diga “Bom dia AnotAI” e faça o seu pedido.",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }

        MicButton(ui) {
            when {
                !micGranted -> requestPerms.launch(PERMISSIONS.map { it.name }.toTypedArray())
                ui.status == Assistant.Status.LISTENING -> assistant.stopListening()
                else -> assistant.listen()
            }
        }

        if (ui.heard.isNotBlank()) Bubble("Você disse", ui.heard, MaterialTheme.colorScheme.surfaceVariant)
        if (ui.reply.isNotBlank()) Bubble("AnotAI", ui.reply, MaterialTheme.colorScheme.primary.copy(alpha = 0.12f))

        if (missing.isNotEmpty()) {
            Section("Permissões") {
                Text(
                    "Para agir no celular, o AnotAI precisa de: " + missing.joinToString(", ") { it.label } + ".",
                    style = MaterialTheme.typography.bodyMedium,
                )
                Button(onClick = { requestPerms.launch(missing.map { it.name }.toTypedArray()) }) {
                    Text("Permitir")
                }
            }
        }

        Section("Escuta contínua (experimental)") {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(
                    "Fica ouvindo “Bom dia/Boa tarde/Boa noite/Oi AnotAI”, com uma notificação fixa.",
                    style = MaterialTheme.typography.bodyMedium,
                    modifier = Modifier.weight(1f),
                )
                Switch(
                    checked = ui.continuous,
                    enabled = micGranted,
                    onCheckedChange = { on -> if (on) WakeService.start(context) else WakeService.stop(context) },
                )
            }
            Text(
                "Gasta mais bateria, e alguns celulares tocam um bipe a cada vez que a escuta reinicia.",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            if (ui.continuous && !overlayGranted) {
                TextButton(onClick = {
                    context.startActivity(
                        Intent(Settings.ACTION_MANAGE_OVERLAY_PERMISSION, Uri.parse("package:${context.packageName}")),
                    )
                }) {
                    Text("Permitir abrir apps com a tela em outro app")
                }
            }
        }

        TypeToTest(assistant)

        Section("Experimente dizer") {
            EXAMPLES.forEach { Text("• $it", style = MaterialTheme.typography.bodyMedium) }
        }

        val notes = remember(ui.notesVersion, resumeTick) { assistant.notes.list() }
        if (notes.isNotEmpty()) {
            Section("Notas") {
                val fmt = DateFormat.getDateTimeInstance(DateFormat.SHORT, DateFormat.SHORT)
                notes.take(10).forEach { note ->
                    Column {
                        Text(note.text, style = MaterialTheme.typography.bodyMedium)
                        Text(
                            fmt.format(Date(note.createdAt)),
                            style = MaterialTheme.typography.labelSmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                }
            }
        }
        Spacer(Modifier.height(24.dp))
    }
}

@Composable
private fun MicButton(ui: Assistant.UiState, onClick: () -> Unit) {
    val listening = ui.status == Assistant.Status.LISTENING
    val color by animateColorAsState(if (listening) Brand.Coral else MaterialTheme.colorScheme.primary, label = "mic")
    val scale by animateFloatAsState(if (listening) 1f + ui.level * 0.18f else 1f, label = "level")
    val status = when (ui.status) {
        Assistant.Status.IDLE -> if (ui.continuous) "Esperando “Oi AnotAI”…" else "Toque para falar"
        Assistant.Status.LISTENING -> "Ouvindo…"
        Assistant.Status.THINKING -> "Entendendo…"
        Assistant.Status.SPEAKING -> "Respondendo…"
    }
    Column(Modifier.fillMaxWidth(), horizontalAlignment = Alignment.CenterHorizontally) {
        Box(
            Modifier
                .size(136.dp)
                .scale(scale)
                .clip(CircleShape)
                .background(color)
                .clickable(onClick = onClick)
                .semantics { contentDescription = if (listening) "Parar de ouvir" else "Falar com o AnotAI" },
            contentAlignment = Alignment.Center,
        ) {
            Icon(
                painterResource(R.drawable.ic_mic),
                contentDescription = null,
                tint = Color.White,
                modifier = Modifier.size(56.dp),
            )
        }
        Spacer(Modifier.height(10.dp))
        Text(status, style = MaterialTheme.typography.titleMedium)
    }
}

@Composable
private fun Bubble(title: String, text: String, background: Color) {
    Card(colors = CardDefaults.cardColors(containerColor = background), modifier = Modifier.fillMaxWidth()) {
        Column(Modifier.padding(14.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
            Text(title, style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
            Text(text, style = MaterialTheme.typography.bodyLarge)
        }
    }
}

@Composable
private fun Section(title: String, content: @Composable () -> Unit) {
    Card(
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surface),
        border = androidx.compose.foundation.BorderStroke(1.dp, MaterialTheme.colorScheme.outline),
        modifier = Modifier.fillMaxWidth(),
    ) {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
            Text(title, style = MaterialTheme.typography.titleSmall, fontWeight = FontWeight.SemiBold)
            content()
        }
    }
}

/** Testa um comando digitando (útil no emulador ou em lugar barulhento). */
@Composable
private fun TypeToTest(assistant: Assistant) {
    var text by remember { mutableStateOf("") }
    Section("Testar digitando") {
        OutlinedTextField(
            value = text,
            onValueChange = { text = it },
            label = { Text("Ex.: Bom dia AnotAI, que horas são?") },
            modifier = Modifier.fillMaxWidth(),
            singleLine = true,
        )
        OutlinedButton(
            onClick = {
                assistant.handleTyped(text)
                text = ""
            },
            enabled = text.isNotBlank(),
        ) {
            Text("Enviar")
        }
    }
}
