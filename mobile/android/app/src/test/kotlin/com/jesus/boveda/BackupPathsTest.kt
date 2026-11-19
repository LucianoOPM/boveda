package com.jesus.boveda

import java.io.File
import java.nio.file.Files
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeNoException
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

class BackupPathsTest {
    @get:Rule
    val folder = TemporaryFolder()

    private lateinit var dir: File

    @Before
    fun setUp() {
        dir = folder.newFolder("backup-tmp")
    }

    @Test
    fun validPathInsideDirIsAccepted() {
        val file = File(dir, "a.bak").apply { writeText("x") }
        assertTrue(isInsideTempDir(dir, file))
    }

    @Test
    fun nonexistentPathInsideDirIsAccepted() {
        assertTrue(isInsideTempDir(dir, File(dir, "no-existe.bak")))
    }

    @Test
    fun nonexistentPathOutsideDirIsRejected() {
        assertFalse(isInsideTempDir(dir, File(folder.root, "no-existe.bak")))
    }

    @Test
    fun dotDotPathIsRejected() {
        folder.newFile("secret.bak")
        assertFalse(isInsideTempDir(dir, File(dir, "../secret.bak")))
    }

    @Test
    fun symlinkPointingOutsideIsRejected() {
        val outside = folder.newFile("outside.bak")
        val link = File(dir, "link.bak")
        try {
            Files.createSymbolicLink(link.toPath(), outside.toPath())
        } catch (e: Exception) {
            assumeNoException(e)
        }
        assertFalse(isInsideTempDir(dir, link))
    }

    @Test
    fun dirItselfIsRejected() {
        assertFalse(isInsideTempDir(dir, dir))
    }

    @Test
    fun subdirectoryOfDirIsRejected() {
        val sub = File(dir, "sub").apply { mkdir() }
        assertFalse(isInsideTempDir(dir, sub))
    }

    @Test
    fun siblingWithSimilarPrefixIsRejected() {
        val sibling = folder.newFolder("backup-tmp2")
        val file = File(sibling, "a.bak").apply { writeText("x") }
        assertFalse(isInsideTempDir(dir, file))
    }
}
